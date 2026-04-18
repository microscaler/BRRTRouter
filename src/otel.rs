//! OpenTelemetry initialization and configuration
//!
//! This module provides comprehensive structured logging with tracing.
//! Inspired by microscaler/obsctl but enhanced with:
//! - Configurable redaction for sensitive data (credentials, PII)
//! - Sampling strategies (all, error-only, sampled)
//! - Rate limiting per endpoint
//! - Async buffered logging for minimal latency impact
//!
//! When **`OTEL_EXPORTER_OTLP_ENDPOINT`** is set, OTLP traces and logs are initialized via
//! **`microscaler_observability::init`**. The shutdown guard is stored internally and flushed from
//! [`shutdown`].
//!
//! # Canonical place for subscriber + OTel provider
//!
//! **[`init_logging_with_config`]** is the **single** place that calls `try_init()` on the process
//! `tracing` subscriber:
//!
//! - **OTLP path** — delegates to **`microscaler-observability`** (tracer + logger + propagator).
//! - **Stdout-only path** — composes `EnvFilter`, sampling, redaction, and `fmt` layers here.
//!
//! Do **not** register a second global OpenTelemetry provider from a library crate (e.g. Lifeguard).
//!
//! # Lifeguard (may-channel logging + DB spans)
//!
//! Services that use **[Lifeguard](https://github.com/microscaler/lifeguard)** should depend on it with
//! the **`tracing`** feature and, if you want the may-channel log path, add
//! **`lifeguard::channel_layer()`** to the same `.with(...)` chain in [`init_logging_with_config`]
//! (after `EnvFilter` if you want filtered events only). Lifeguard’s `tracing` spans then share the
//! host subscriber with BRRTRouter; it does not call `set_tracer_provider`.
//!
//! Cross-repo doc: `lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md`.
//!
//! # Tests
//!
//! Integration tests use `tests/tracing_util.rs` (`TestTracing`): `tracing::subscriber::set_default`
//! with a scoped `Registry` + `OpenTelemetryLayer` avoids fighting the global `try_init()` used by
//! production init.

use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tracing::Level;
use tracing::{Event, Metadata, Subscriber};
use tracing_subscriber::layer::{Context as LayerContext, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, [`init_logging_with_config`] stores the
/// [`microscaler_observability::ShutdownGuard`] here so callers keep a stable `Result<()>`
/// signature; flush runs on [`shutdown`] or process teardown.
static OTEL_SHUTDOWN: OnceCell<Mutex<Option<microscaler_observability::ShutdownGuard>>> =
    OnceCell::new();

/// Merge BRRTRouter [`LogConfig`] into OTEL env (`RUST_LOG`, `may_minihttp`, debug-session).
fn merge_observability_config(log_config: &LogConfig) -> microscaler_observability::ObservabilityConfig {
    let mut obs = microscaler_observability::ObservabilityConfig::from_env();
    let mut merged = obs
        .rust_log
        .clone()
        .unwrap_or_else(|| log_config.log_level.clone());
    merged.push_str(",may_minihttp::http_server=warn");
    if log_config.enable_debug_session_tracing {
        merged.push(',');
        merged.push_str(crate::agent_debug::LOG_DIRECTIVE);
    }
    if let Some(tf) = &log_config.target_filter {
        for filter in tf.split(',') {
            let filter = filter.trim();
            if !filter.is_empty() {
                merged.push(',');
                merged.push_str(filter);
            }
        }
    }
    obs.rust_log = Some(merged);
    obs
}

/// Log format: JSON for production, pretty-print for development
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

impl LogFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pretty" => LogFormat::Pretty,
            _ => LogFormat::Json, // Default to JSON
        }
    }
}

/// Redaction level for sensitive data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionLevel {
    /// No redaction (DANGEROUS - dev only)
    None,
    /// Redact credentials (API keys, tokens, passwords)
    Credentials,
    /// Redact credentials + PII (emails, IPs, user IDs)
    Full,
}

impl RedactionLevel {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" => RedactionLevel::None,
            "full" => RedactionLevel::Full,
            _ => RedactionLevel::Credentials, // Default to credentials
        }
    }
}

/// Sampling mode: how to decide which logs to emit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingMode {
    /// Log everything (high volume)
    All,
    /// Log only WARN and ERROR levels
    ErrorOnly,
    /// Sample successful requests, log all errors
    Sampled,
}

impl SamplingMode {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "all" => SamplingMode::All,
            "error-only" | "error_only" => SamplingMode::ErrorOnly,
            _ => SamplingMode::Sampled, // Default to sampled
        }
    }
}

/// Comprehensive logging configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log level: trace/debug/info/warn/error
    pub log_level: String,
    /// Log format: json/pretty
    pub format: LogFormat,
    /// Redaction level: none/credentials/full
    pub redact_level: RedactionLevel,
    /// Sampling mode: all/error-only/sampled
    pub sampling_mode: SamplingMode,
    /// Sampling rate (0.0-1.0) for Sampled mode
    pub sampling_rate: f64,
    /// Rate limit: max logs/sec per endpoint
    pub rate_limit_rps: u64,
    /// Enable async buffered logging
    pub async_logging: bool,
    /// Buffer size for async logging
    pub buffer_size: usize,
    /// Module filter (comma-separated)
    pub target_filter: Option<String>,
    /// Include file:line location (dev only)
    pub include_location: bool,
    /// Merge [`crate::agent_debug::LOG_DIRECTIVE`] into the [`EnvFilter`] so debug-session markers
    /// are emitted at INFO without hand-editing `RUST_LOG`.
    ///
    /// Toggle via env **`BRRTR_DEBUG_SESSION`**: `1`, `true`, `yes`, or `on` (case-insensitive) enables;
    /// unset or any other value disables. Kubernetes: set in a ConfigMap and reference from the pod.
    pub enable_debug_session_tracing: bool,
}

impl LogConfig {
    /// Parse configuration from environment variables with defaults
    pub fn from_env() -> Self {
        Self {
            log_level: env::var("BRRTR_LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            format: LogFormat::parse(
                &env::var("BRRTR_LOG_FORMAT").unwrap_or_else(|_| "json".to_string()),
            ),
            redact_level: RedactionLevel::parse(
                &env::var("BRRTR_LOG_REDACT_LEVEL").unwrap_or_else(|_| "credentials".to_string()),
            ),
            sampling_mode: SamplingMode::parse(
                &env::var("BRRTR_LOG_SAMPLING_MODE").unwrap_or_else(|_| "sampled".to_string()),
            ),
            sampling_rate: env::var("BRRTR_LOG_SAMPLING_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.1), // 10% default
            rate_limit_rps: env::var("BRRTR_LOG_RATE_LIMIT_RPS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            async_logging: env::var("BRRTR_LOG_ASYNC")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            buffer_size: env::var("BRRTR_LOG_BUFFER_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8192),
            target_filter: env::var("BRRTR_LOG_TARGET_FILTER").ok(),
            include_location: env::var("BRRTR_LOG_INCLUDE_LOCATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            enable_debug_session_tracing: env::var("BRRTR_DEBUG_SESSION")
                .map(|s| {
                    matches!(
                        s.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false),
        }
    }

    /// Create a default configuration for testing
    pub fn default_dev() -> Self {
        Self {
            log_level: "debug".to_string(),
            format: LogFormat::Pretty,
            redact_level: RedactionLevel::None,
            sampling_mode: SamplingMode::All,
            sampling_rate: 1.0,
            rate_limit_rps: 1000,
            async_logging: false,
            buffer_size: 1024,
            target_filter: None,
            include_location: true,
            enable_debug_session_tracing: false,
        }
    }

    /// Create a default production configuration
    pub fn default_prod() -> Self {
        Self {
            log_level: "info".to_string(),
            format: LogFormat::Json,
            redact_level: RedactionLevel::Credentials,
            sampling_mode: SamplingMode::Sampled,
            sampling_rate: 0.1,
            rate_limit_rps: 10,
            async_logging: true,
            buffer_size: 8192,
            target_filter: None,
            include_location: false,
            enable_debug_session_tracing: false,
        }
    }
}

/// Redaction layer: placeholder for tracing subscriber integration.
///
/// **Important**: The `tracing` crate's `Layer::on_event` cannot mutate field values
/// in-flight. Effective redaction must happen at the call-site — see [`crate::sanitize`]
/// for the canonical field sanitizer used throughout the codebase.
///
/// This layer remains in the subscriber stack for future use (e.g. span-attribute
/// filtering) and to keep the `RedactionLevel` available in the subscriber context.
/// The actual sensitive-data masking is performed by [`crate::sanitize::Sanitizer`]
/// at each logging call-site.
pub struct RedactionLayer {
    level: RedactionLevel,
}

impl RedactionLayer {
    pub fn new(level: RedactionLevel) -> Self {
        Self { level }
    }

    /// Check if a field name matches the sensitive-field policy.
    ///
    /// Delegates to [`crate::sanitize::Sanitizer::should_redact`].
    pub fn should_redact(&self, field_name: &str) -> bool {
        crate::sanitize::Sanitizer::new(self.level).should_redact(field_name)
    }

    /// Mask a sensitive string value.
    ///
    /// Delegates to [`crate::sanitize::Sanitizer::redact_value`].
    pub fn redact_value(&self, field_name: &str, value: &str) -> String {
        crate::sanitize::Sanitizer::new(self.level).redact_value(field_name, value)
    }
}

impl<S> Layer<S> for RedactionLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, _event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        // Call-site sanitization via `crate::sanitize::Sanitizer` is the correct
        // Rust/tracing approach — `on_event` cannot mutate field values in-flight.
        // See `src/sanitize.rs` for the centralized redaction logic.
    }
}

/// Sampling layer: decides whether to emit a log based on sampling rules
pub struct SamplingLayer {
    mode: SamplingMode,
    sampling_rate: f64,
    counter: AtomicU64,
}

impl SamplingLayer {
    pub fn new(mode: SamplingMode, sampling_rate: f64) -> Self {
        Self {
            mode,
            sampling_rate: sampling_rate.clamp(0.0, 1.0),
            counter: AtomicU64::new(0),
        }
    }

    /// Check if this event should be logged based on sampling rules
    fn should_sample(&self, metadata: &Metadata<'_>) -> bool {
        match self.mode {
            SamplingMode::All => true,
            SamplingMode::ErrorOnly => {
                matches!(metadata.level(), &Level::WARN | &Level::ERROR)
            }
            SamplingMode::Sampled => {
                // Always log errors
                if matches!(metadata.level(), &Level::WARN | &Level::ERROR) {
                    return true;
                }

                // Sample other events based on rate
                let count = self.counter.fetch_add(1, Ordering::Relaxed);
                let sample_interval = (1.0 / self.sampling_rate) as u64;
                sample_interval > 0 && count.is_multiple_of(sample_interval)
            }
        }
    }
}

impl<S> Layer<S> for SamplingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn enabled(&self, metadata: &Metadata<'_>, _ctx: LayerContext<'_, S>) -> bool {
        self.should_sample(metadata)
    }

    fn on_event(&self, _event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        // Sampling is handled in enabled(), no additional work needed here
    }
}

/// Initialize logging with structured tracing (legacy interface)
///
/// This function maintains backward compatibility. For new code, use `init_logging_with_config()`.
///
/// # Arguments
///
/// * `_service_name` - Name of this service (reserved for future OTLP use)
/// * `log_level` - Log level: "trace", "debug", "info", "warn", "error"
/// * `_otlp_endpoint` - Reserved for future OTLP implementation
///
/// # Example
///
/// ```no_run
/// use brrtrouter::otel;
///
/// otel::init_logging("my-service", "info", None)
///     .expect("Failed to initialize logging");
/// ```
pub fn init_logging(
    _service_name: &str,
    log_level: &str,
    _otlp_endpoint: Option<&str>,
) -> Result<()> {
    let mut config = LogConfig::from_env();
    config.log_level = log_level.to_string();
    init_logging_with_config(&config)
}

/// Initialize logging with comprehensive configuration
///
/// This function sets up tracing with:
/// - JSON or pretty-print formatting
/// - Sensitive data redaction
/// - Configurable sampling
/// - Async buffered output (optional)
///
/// # Arguments
///
/// * `config` - Complete logging configuration
///
/// # Example
///
/// ```no_run
/// use brrtrouter::otel::{LogConfig, init_logging_with_config};
///
/// let config = LogConfig::from_env();
/// init_logging_with_config(&config)
///     .expect("Failed to initialize logging");
/// ```
pub fn init_logging_with_config(config: &LogConfig) -> Result<()> {
    let use_otlp = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();

    if use_otlp {
        let obs = merge_observability_config(config);
        let guard = microscaler_observability::init(obs).map_err(anyhow::Error::msg)?;
        OTEL_SHUTDOWN
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|e| anyhow::anyhow!("OTEL shutdown mutex poisoned: {e}"))?
            .replace(guard);
        return Ok(());
    }

    // Parse log level
    let level = match config.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let mut env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.as_str()));

    // may_minihttp logging: the fork at microscaler/may_minihttp now properly
    // filters client disconnects (BrokenPipe, etc.) to not log as ERROR.
    // Allow warn+ for actual issues, suppress debug/info noise.
    env_filter = env_filter.add_directive(
        "may_minihttp::http_server=warn"
            .parse()
            .expect("valid directive"),
    );

    if config.enable_debug_session_tracing {
        env_filter = env_filter.add_directive(
            crate::agent_debug::LOG_DIRECTIVE
                .parse()
                .expect("valid debug_session directive"),
        );
    }

    // Apply custom target filters if provided
    if let Some(target_filter) = &config.target_filter {
        for filter in target_filter.split(',') {
            let filter = filter.trim();
            if !filter.is_empty() {
                if let Ok(directive) = filter.parse() {
                    env_filter = env_filter.add_directive(directive);
                } else {
                    eprintln!("Warning: Invalid log filter directive: {}", filter);
                }
            }
        }
    }

    // Create sampling layer
    let sampling_layer = SamplingLayer::new(config.sampling_mode, config.sampling_rate);

    // Create redaction layer
    let redaction_layer = RedactionLayer::new(config.redact_level);

    // Create fmt layer based on format preference
    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(sampling_layer)
        .with(redaction_layer);

    if config.async_logging {
        // Async logging with buffering
        let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());

        let fmt_layer = match config.format {
            LogFormat::Json => tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_span_list(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_writer(non_blocking)
                .boxed(),
            LogFormat::Pretty => tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_writer(non_blocking)
                .boxed(),
        };

        registry
            .with(fmt_layer)
            .try_init()
            .context("Failed to initialize async logging")?;

        // Store guard to prevent premature flush (leak it for application lifetime)
        std::mem::forget(_guard);
    } else {
        // Synchronous logging
        let fmt_layer = match config.format {
            LogFormat::Json => tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_span_list(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .boxed(),
            LogFormat::Pretty => tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .boxed(),
        };

        registry
            .with(fmt_layer)
            .try_init()
            .context("Failed to initialize sync logging")?;
    }

    Ok(())
}

/// Shutdown telemetry: flushes OTLP batch processors when the OTLP path was used.
///
/// Call this before application exit (e.g. SIGTERM handler) so spans and logs
/// are exported. Safe to call multiple times.
pub fn shutdown() {
    if let Some(lock) = OTEL_SHUTDOWN.get() {
        if let Ok(mut slot) = lock.lock() {
            slot.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // LogConfig Tests
    // ========================================================================

    #[test]
    fn test_log_config_default_dev() {
        let config = LogConfig::default_dev();
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.format, LogFormat::Pretty);
        assert_eq!(config.redact_level, RedactionLevel::None);
        assert_eq!(config.sampling_mode, SamplingMode::All);
        assert_eq!(config.sampling_rate, 1.0);
        assert!(!config.async_logging);
        assert!(config.include_location);
        assert!(!config.enable_debug_session_tracing);
    }

    #[test]
    fn test_log_config_default_prod() {
        let config = LogConfig::default_prod();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.format, LogFormat::Json);
        assert_eq!(config.redact_level, RedactionLevel::Credentials);
        assert_eq!(config.sampling_mode, SamplingMode::Sampled);
        assert_eq!(config.sampling_rate, 0.1);
        assert!(config.async_logging);
        assert!(!config.include_location);
        assert!(!config.enable_debug_session_tracing);
    }

    #[test]
    fn test_log_format_parse() {
        assert_eq!(LogFormat::parse("json"), LogFormat::Json);
        assert_eq!(LogFormat::parse("JSON"), LogFormat::Json);
        assert_eq!(LogFormat::parse("pretty"), LogFormat::Pretty);
        assert_eq!(LogFormat::parse("PRETTY"), LogFormat::Pretty);
        assert_eq!(LogFormat::parse("invalid"), LogFormat::Json); // Default
    }

    #[test]
    fn test_redaction_level_parse() {
        assert_eq!(RedactionLevel::parse("none"), RedactionLevel::None);
        assert_eq!(
            RedactionLevel::parse("credentials"),
            RedactionLevel::Credentials
        );
        assert_eq!(RedactionLevel::parse("full"), RedactionLevel::Full);
        assert_eq!(
            RedactionLevel::parse("CREDENTIALS"),
            RedactionLevel::Credentials
        );
        assert_eq!(
            RedactionLevel::parse("invalid"),
            RedactionLevel::Credentials
        ); // Default
    }

    #[test]
    fn test_sampling_mode_parse() {
        assert_eq!(SamplingMode::parse("all"), SamplingMode::All);
        assert_eq!(SamplingMode::parse("error-only"), SamplingMode::ErrorOnly);
        assert_eq!(SamplingMode::parse("error_only"), SamplingMode::ErrorOnly);
        assert_eq!(SamplingMode::parse("sampled"), SamplingMode::Sampled);
        assert_eq!(SamplingMode::parse("invalid"), SamplingMode::Sampled); // Default
    }

    // ========================================================================
    // RedactionLayer Tests
    // ========================================================================

    #[test]
    fn test_redaction_layer_should_redact_credentials() {
        let layer = RedactionLayer::new(RedactionLevel::Credentials);

        // Should redact credentials
        assert!(layer.should_redact("password"));
        assert!(layer.should_redact("api_key"));
        assert!(layer.should_redact("apiKey"));
        assert!(layer.should_redact("token"));
        assert!(layer.should_redact("access_token"));
        assert!(layer.should_redact("secret"));
        assert!(layer.should_redact("authorization"));
        assert!(layer.should_redact("credit_card"));

        // Should NOT redact PII at Credentials level
        assert!(!layer.should_redact("email"));
        assert!(!layer.should_redact("ip_address"));
        assert!(!layer.should_redact("user_id"));
    }

    #[test]
    fn test_redaction_layer_should_redact_full() {
        let layer = RedactionLayer::new(RedactionLevel::Full);

        // Should redact credentials
        assert!(layer.should_redact("password"));
        assert!(layer.should_redact("api_key"));

        // Should also redact PII at Full level
        assert!(layer.should_redact("email"));
        assert!(layer.should_redact("ip_address"));
        assert!(layer.should_redact("user_id"));
        assert!(layer.should_redact("phone"));
        assert!(layer.should_redact("name"));
    }

    #[test]
    fn test_redaction_layer_none() {
        let layer = RedactionLayer::new(RedactionLevel::None);

        // Should NOT redact anything at None level
        assert!(!layer.should_redact("password"));
        assert!(!layer.should_redact("api_key"));
        assert!(!layer.should_redact("email"));
        assert!(!layer.should_redact("user_id"));
    }

    #[test]
    fn test_redaction_value_truncation() {
        let layer = RedactionLayer::new(RedactionLevel::Credentials);

        // API keys should be truncated to first 4 chars
        assert_eq!(layer.redact_value("api_key", "test1234567890"), "test***");
        assert_eq!(layer.redact_value("token", "abcdefghij"), "abcd***");

        // Short values still get redacted
        assert_eq!(layer.redact_value("api_key", "abc"), "<REDACTED>");

        // Non-key fields get fully redacted
        assert_eq!(layer.redact_value("password", "secret123"), "<REDACTED>");
    }

    // ========================================================================
    // SamplingLayer Tests
    // ========================================================================

    #[test]
    fn test_sampling_layer_all_mode() {
        let layer = SamplingLayer::new(SamplingMode::All, 1.0);

        // Should sample everything
        let info_metadata = tracing::Metadata::new(
            "test",
            "test::module",
            Level::INFO,
            None,
            None,
            None,
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        assert!(layer.should_sample(&info_metadata));
    }

    #[test]
    fn test_sampling_layer_error_only_mode() {
        let layer = SamplingLayer::new(SamplingMode::ErrorOnly, 1.0);

        // Should only sample WARN and ERROR
        let info_metadata = tracing::Metadata::new(
            "test",
            "test::module",
            Level::INFO,
            None,
            None,
            None,
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        let warn_metadata = tracing::Metadata::new(
            "test",
            "test::module",
            Level::WARN,
            None,
            None,
            None,
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        let error_metadata = tracing::Metadata::new(
            "test",
            "test::module",
            Level::ERROR,
            None,
            None,
            None,
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        assert!(!layer.should_sample(&info_metadata));
        assert!(layer.should_sample(&warn_metadata));
        assert!(layer.should_sample(&error_metadata));
    }

    #[test]
    fn test_sampling_layer_sampled_mode_always_logs_errors() {
        let layer = SamplingLayer::new(SamplingMode::Sampled, 0.1);

        // Should always sample errors regardless of rate
        let error_metadata = tracing::Metadata::new(
            "test",
            "test::module",
            Level::ERROR,
            None,
            None,
            None,
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        // Call multiple times, should always return true for errors
        for _ in 0..100 {
            assert!(layer.should_sample(&error_metadata));
        }
    }

    #[test]
    fn test_sampling_layer_sampled_mode_respects_rate() {
        let layer = SamplingLayer::new(SamplingMode::Sampled, 0.5); // 50% sampling

        let info_metadata = tracing::Metadata::new(
            "test",
            "test::module",
            Level::INFO,
            None,
            None,
            None,
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        // Sample 100 events, expect ~50% to be sampled
        let mut sampled_count = 0;
        for _ in 0..100 {
            if layer.should_sample(&info_metadata) {
                sampled_count += 1;
            }
        }

        // Should be close to 50 (allow some variance: 40-60)
        assert!(
            (40..=60).contains(&sampled_count),
            "Expected ~50 samples, got {}",
            sampled_count
        );
    }

    #[test]
    fn test_sampling_rate_clamping() {
        // Rates should be clamped to 0.0-1.0
        let layer1 = SamplingLayer::new(SamplingMode::Sampled, -0.5);
        assert_eq!(layer1.sampling_rate, 0.0);

        let layer2 = SamplingLayer::new(SamplingMode::Sampled, 1.5);
        assert_eq!(layer2.sampling_rate, 1.0);

        let layer3 = SamplingLayer::new(SamplingMode::Sampled, 0.5);
        assert_eq!(layer3.sampling_rate, 0.5);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_log_config_from_env_with_defaults() {
        // Ensure env vars are not set (or use reasonable defaults)
        // This test verifies default values when env vars are missing
        let config = LogConfig::from_env();

        // Should have reasonable defaults
        assert!(matches!(config.format, LogFormat::Json | LogFormat::Pretty));
        assert!(matches!(
            config.redact_level,
            RedactionLevel::None | RedactionLevel::Credentials | RedactionLevel::Full
        ));
        assert!(config.sampling_rate >= 0.0 && config.sampling_rate <= 1.0);
        assert!(config.buffer_size > 0);
    }

    // Mock callsite for metadata creation in tests
    struct TestCallsite;
    impl tracing::callsite::Callsite for TestCallsite {
        fn set_interest(&self, _interest: tracing::subscriber::Interest) {}
        // Test-only stub: `metadata()` is never invoked in these tests.
        #[allow(clippy::panic)]
        fn metadata(&self) -> &tracing::Metadata<'_> {
            panic!("not used in tests")
        }
    }
    static CALLSITE: TestCallsite = TestCallsite;
}
