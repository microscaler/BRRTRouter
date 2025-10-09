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
//! - Decimal: `16384` (16 KB)
//! - Hexadecimal: `0x4000` (16 KB)
//!
//! Default: `0x4000` (16 KB)
//!
//! **Why this matters:**
//! - Larger stacks support deeper call chains and larger local variables
//! - Smaller stacks reduce memory usage for many concurrent coroutines
//! - 800 concurrent requests × 1 MB stack = 800 MB virtual memory
//! - Tune based on your handler complexity and concurrency needs
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::runtime_config::RuntimeConfig;
//!
//! let config = RuntimeConfig::from_env();
//! println!("Stack size: {} bytes", config.stack_size);
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
//! - Simple handlers: `0x4000` (16 KB)
//! - Complex logic: `0x8000` (32 KB)
//! - Deep recursion: `0x10000` (64 KB)

use std::env;

/// Runtime configuration loaded from environment variables.
///
/// Load this at startup using [`RuntimeConfig::from_env()`] to configure
/// the coroutine runtime behavior.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    /// Stack size for coroutines in bytes (default: 16 KB / 0x4000)
    pub stack_size: usize,
}

impl RuntimeConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        let stack_size = match env::var("BRRTR_STACK_SIZE") {
            Ok(val) => {
                if let Some(hex) = val.strip_prefix("0x") {
                    usize::from_str_radix(hex, 16).unwrap_or(0x4000)
                } else {
                    val.parse().unwrap_or(0x4000)
                }
            }
            Err(_) => 0x4000,
        };
        RuntimeConfig { stack_size }
    }
}
