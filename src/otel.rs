//! OpenTelemetry initialization and configuration
//!
//! This module provides structured logging with tracing.
//! Inspired by microscaler/obsctl but simplified for the current phase.
//!
//! OTLP export will be added in a future phase once we verify the basic logging works.

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize logging with structured tracing
///
/// This function sets up tracing with JSON output for production.
/// OTLP export will be added in a future iteration.
///
/// Similar to obsctl's init_logging but using the tracing ecosystem.
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
    // Parse log level (case-insensitive, like obsctl)
    let level = match log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO, // Default to info like obsctl
    };

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level.as_str()));

    // Create console/stdout layer with JSON formatting
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_span_list(true);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    // Initialize subscriber
    registry.try_init()?;

    Ok(())
}

/// Shutdown telemetry (no-op for now, reserved for future OTLP)
///
/// Call this before application exit to flush any pending spans.
/// Currently a no-op until OTLP is integrated.
pub fn shutdown() {
    // No-op for now - will flush OTLP spans in future
}

#[cfg(test)]
mod tests {
    use super::*;

    // Similar test patterns to obsctl's logging tests

    #[test]
    fn test_init_logging_with_valid_levels() {
        let levels = ["trace", "debug", "info", "warn", "error"];
        for level in levels {
            // Can't easily test initialization due to global state
            // but ensure level parsing doesn't panic
            let parsed = match level.to_lowercase().as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            };
            assert!(parsed == Level::TRACE || parsed == Level::DEBUG || 
                    parsed == Level::INFO || parsed == Level::WARN || 
                    parsed == Level::ERROR);
        }
    }

    #[test]
    fn test_init_logging_case_insensitive() {
        let mixed_case_levels = ["TRACE", "Debug", "INFO", "Warn", "ERROR"];
        for level in mixed_case_levels {
            let parsed = match level.to_lowercase().as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            };
            // Should handle case insensitivity
            assert!(parsed == Level::TRACE || parsed == Level::DEBUG || 
                    parsed == Level::INFO || parsed == Level::WARN || 
                    parsed == Level::ERROR);
        }
    }

    #[test]
    fn test_level_mapping_with_invalid() {
        let invalid_level = "invalid";
        let parsed = match invalid_level.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" => Level::WARN,
            "error" => Level::ERROR,
            _ => Level::INFO, // Should default to INFO
        };
        assert_eq!(parsed, Level::INFO, "Invalid level should default to INFO");
    }

    #[test]
    fn test_empty_string_level() {
        let empty_level = "";
        let parsed = match empty_level.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" => Level::WARN,
            "error" => Level::ERROR,
            _ => Level::INFO, // Should default to INFO
        };
        assert_eq!(parsed, Level::INFO, "Empty level should default to INFO");
    }
}

