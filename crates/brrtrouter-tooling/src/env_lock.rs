//! Test support: serialize tests that mutate process-global env vars.
//!
//! Cargo runs unit tests in one binary on parallel threads, and env vars are
//! process-global, so tests that `set_var`/`remove_var` race each other —
//! including across modules (e.g. `paths` sets `BRRTROUTER_ROOT` while `build`
//! reads `CARGO_TARGET_ARCH`). Any test touching env vars must hold this lock
//! for its whole body.

use std::sync::{Mutex, OnceLock};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Acquire the process-wide env lock; return a guard to hold for the test body.
pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}
