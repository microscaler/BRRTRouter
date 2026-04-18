//! # Runtime Configuration Module
//!
//! The runtime configuration module provides environment variable-based configuration
//! for BRRTRouter's runtime behavior.
//!
//! ## Overview
//!
//! This module loads configuration from environment variables that affect:
//! - Coroutine stack sizes
//! - Performance tuning
//! - Runtime behavior
//!
//! ## Environment Variables
//!
//! ### `BRRTR_STACK_SIZE`
//!
//! Sets the stack size for coroutine handlers. Accepts values in:
//! - Decimal: `65536` (64 KB)
//! - Hexadecimal: `0x10000` (64 KB)
//!
//! Default: `0x8000` (32 KB) - Optimal for typical handlers with ~4x safety margin
//!
//! **Why this matters:**
//! - Larger stacks support deeper call chains and larger local variables
//! - Smaller stacks reduce memory usage for many concurrent coroutines
//! - 4,500 concurrent requests × 32 KB stack = 72 MB virtual memory
//! - Typical handler uses ~3.5 KB; 32 KB provides 4x safety margin
//! - Tune based on your handler complexity and concurrency needs
//!
//! ### `BRRTR_MAY_WORKERS`
//!
//! Sets the **may** scheduler worker thread count. Must be called before the first `go!` /
//! coroutine (see `may::config::set_workers`).
//!
//! **Why this matters with Lifeguard + `may_postgres`:** HTTP handlers run on may workers and
//! block on `PooledLifeExecutor` reply channels. Pool threads run queries that schedule
//! `may_postgres` I/O as `go!` coroutines on the **same** global may pool. If every may worker is
//! blocked waiting for Postgres, no thread runs connection I/O — requests hang forever.
//!
//! Default when unset: `max(32, available_parallelism + DB_POOL_MAX + 16)` where `DB_POOL_MAX`
//! defaults to `10` if unset (matching typical Lifeguard pool sizing).
//!
//! Override example: `export BRRTR_MAY_WORKERS=64`
//!
//! ### `BRRTR_SCHEMA_CACHE`
//!
//! Controls whether JSON Schema validators are cached across requests.
//! Accepts values: `on`, `off`, `true`, `false`, `1`, `0`
//!
//! Default: `on` (enabled)
//!
//! **Why this matters:**
//! - Eliminates per-request schema compilation overhead
//! - Significantly reduces CPU usage under high load
//! - Reduces memory allocations for validation
//! - Can be disabled for debugging or if issues arise
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::runtime_config::RuntimeConfig;
//!
//! let config = RuntimeConfig::from_env();
//! println!("Stack size: {} bytes, may workers: {}", config.stack_size, config.may_workers);
//! ```
//!
//! ## Example Configuration
//!
//! ```bash
//! # Set stack size to 32 KB
//! export BRRTR_STACK_SIZE=0x8000
//!
//! # Or in decimal
//! export BRRTR_STACK_SIZE=32768
//!
//! # Start your service
//! cargo run
//! ```
//!
//! ## Performance Impact
//!
//! Stack size affects:
//! - **Memory usage**: Total = stack_size × concurrent_coroutines
//! - **Allocation speed**: Larger stacks may take longer to allocate
//! - **Stack overflows**: Too small causes panics; too large wastes memory
//!
//! Recommended values:
//! - Simple handlers: `0x2000` (8 KB)
//! - Typical handlers: `0x8000` (32 KB) ← **default, validated at 4,500 concurrent users**
//! - Complex logic: `0x8000` (32 KB)
//! - Deep recursion: `0x10000` (64 KB)

use std::env;

/// Runtime configuration loaded from environment variables.
///
/// Load this at startup using [`RuntimeConfig::from_env()`] to configure
/// the coroutine runtime behavior.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    /// Stack size for coroutines in bytes (default: 32 KB / 0x8000)
    /// Optimal for typical handlers (~3.5 KB used) with 4x safety margin
    pub stack_size: usize,
    /// Whether to cache JSON Schema validators (default: true)
    /// Eliminates per-request schema compilation overhead
    pub schema_cache_enabled: bool,
    /// May scheduler worker threads (`may::config().set_workers`). Minimum 2 when applied.
    pub may_workers: usize,
}

impl RuntimeConfig {
    /// Load configuration from environment variables.
    ///
    /// **IMPORTANT**: Stack sizes are made odd (if even) to enable May's internal
    /// stack usage tracking. This allows us to measure actual stack usage, not just
    /// allocation size.
    pub fn from_env() -> Self {
        let mut stack_size = match env::var("BRRTR_STACK_SIZE") {
            Ok(val) => {
                if let Some(hex) = val.strip_prefix("0x") {
                    usize::from_str_radix(hex, 16).unwrap_or(0x8000)
                } else {
                    val.parse().unwrap_or(0x8000)
                }
            }
            Err(_) => 0x8000, // 32KB default - optimal for typical handlers with 4x safety margin
        };

        // Phase 2.2 hygiene: `may`'s stack-usage tracking is opt-in via an
        // odd stack size, which triggers a raw `println!` **per coroutine**
        // on every spawn (see `may::coroutine_impl`). This output is *not*
        // routed through `tracing` — it bypasses Promtail/Loki entirely
        // and lands as unstructured text directly on stdout.
        //
        // Under 2000u load that's thousands of synchronous stdout writes
        // per second — pure bench debris with no operational value, and
        // the write-lock contention on the stdout FD was a measurable
        // contributor to the SIGABRT we saw before this fix. Gate behind
        // `BRRTR_TRACK_STACK_USAGE=1` so it stays available for deliberate
        // stack-tuning sessions but does not fire in production or benches
        // by default.
        if env::var("BRRTR_TRACK_STACK_USAGE")
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
            && stack_size.is_multiple_of(2)
        {
            stack_size += 1;
            eprintln!(
                "[telemetry] Adjusted stack size to {stack_size} (odd) to enable usage tracking"
            );
        }

        // Parse schema cache configuration (default: enabled)
        let schema_cache_enabled = match env::var("BRRTR_SCHEMA_CACHE") {
            Ok(val) => {
                let val_lower = val.to_lowercase();
                !matches!(val_lower.as_str(), "off" | "false" | "0" | "no")
            }
            Err(_) => true, // Default to enabled
        };

        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8)
            .max(1);
        let db_pool_max = env::var("DB_POOL_MAX")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(10);
        let default_may_workers = (cpus + db_pool_max + 16).max(32);
        let may_workers = match env::var("BRRTR_MAY_WORKERS") {
            Ok(val) => val.parse::<usize>().unwrap_or(default_may_workers).max(2),
            Err(_) => default_may_workers.max(2),
        };

        RuntimeConfig {
            stack_size,
            schema_cache_enabled,
            may_workers,
        }
    }
}
