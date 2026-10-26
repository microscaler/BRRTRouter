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
//! The stack of `may`'s own coroutines, **in bytes**: the HTTP connection coroutines that parse, route, validate and
//! dispatch. Not the handlers' stacks: those come from the spec (`x-brrtrouter-stack-size`) and
//! `BRRTR_STACK_SIZE__<HANDLER>`, see `crate::handler_stack`. Accepts what `handler_stack::parse_size` does:
//! `262144`, `0x40000`, `256KiB`, `2MiB`.
//!
//! Default: 256 KiB. `may` takes machine words, so [`RuntimeConfig::stack_size`] holds the converted value
//! (`bytes / size_of::<usize>()`) and [`RuntimeConfig::stack_size_bytes`] the bytes.
//!
//! Until 25 Oct 2026 the variable was passed to `may` unconverted, so a value was read as **words** (8 bytes each on
//! 64-bit): `0x40000` gave 2 MiB stacks, not 256 KiB. Deployments that set it were moved to the byte value of what
//! they actually ran (`0x40000` → `2MiB`, `0x200000` → `16MiB`); the unset default (256 KiB) is unchanged.
//!
//! Stacks are mapped lazily: a bigger stack costs address space, and resident memory only for the pages a coroutine
//! touches.
//!
//! ### `BRRTR_COROUTINE_POOL_CAPACITY`
//!
//! Finished coroutines `may` keeps for reuse. Default 100. `may`'s own default is 1,000, allocated up front when its
//! scheduler starts, each keeping the stack pages it last touched. BRRTRouter applies this before it spawns its first
//! coroutine (see [`ensure_coroutine_pool_capacity`]), so a service gets it without calling
//! `may::config().set_pool_capacity` itself; a capacity the service already set is left alone.
//!
//! ### `BRRTR_MAY_POLL_TIMEOUT_MS`
//!
//! How long an idle `may` worker waits in `epoll_wait` before it looks for work again (collects its global queue and
//! tries to steal from other workers). Default **50 ms**; `may`'s own default is 10 ms. An idle service wakes every
//! worker that often: with the default 32+ workers, 10 ms is 3,200 wake-ups a second doing nothing, ~75 mcores per pod
//! in pp (25 Oct 2026). I/O deadlines are unaffected (the wait is cut short for the next I/O timer) and work handed to
//! a worker from outside it wakes that worker; only idle workers' stealing gets less frequent. Applied by
//! [`RuntimeConfig::from_env`] (see [`ensure_may_poll_timeout`]) and [`RuntimeConfig::apply`]; a service that set
//! `may::config().set_timeout_ns` itself keeps its value unless this variable is set.
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
//! ### `BRRTR_JWKS_FETCH_TIMEOUT_MS`
//!
//! Per-attempt HTTP timeout for JWKS fetches, in whole milliseconds. Applies to both
//! `security::JwksBearerProvider` and `security::SpiffeProvider`; each also exposes a builder
//! (`fetch_timeout` / `jwks_fetch_timeout`) for per-provider overrides.
//!
//! Default: `3000` (see `security::DEFAULT_JWKS_FETCH_TIMEOUT`).
//!
//! **Why this matters:**
//! - The value used to be hard-coded at 200ms, sized for plaintext in-cluster
//!   `http://…svc.cluster.local` JWKS URLs — one pod-network hop, no handshake.
//! - Consumers now use `https://` on real hostnames behind TLS-terminating edges, where a fetch
//!   pays DNS + TCP connect + TLS handshake + chain verification before the HTTP round trip.
//! - Too low and *every* refresh times out: no keyset is cached and every JWT is rejected with a
//!   bare 401. Two attempts are made per refresh, so worst-case refresh time is ≈ `2 × timeout`.
//! - Refreshes are cached and debounced, so a generous timeout costs far less than a failed fetch.
//!
//! Read at provider construction (startup), not per request. Unset, unparseable or zero values
//! fall back to the default and are reported at `WARN`.
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::runtime_config::RuntimeConfig;
//!
//! let config = RuntimeConfig::from_env();
//! println!("Stack size: {} bytes, may workers: {}", config.stack_size_bytes(), config.may_workers);
//! config.apply(); // may's stack size, workers and coroutine pool
//! ```
//!
//! ## Example Configuration
//!
//! ```bash
//! # Connection coroutine stacks of 512 KiB
//! export BRRTR_STACK_SIZE=512KiB
//!
//! # Or in hex / decimal bytes
//! export BRRTR_STACK_SIZE=0x80000
//! export BRRTR_STACK_SIZE=524288
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
//! The default (256 KiB) has run every Microscaler service; raise it only for a measured overflow.

use std::env;

/// Runtime configuration loaded from environment variables.
///
/// Load this at startup using [`RuntimeConfig::from_env()`] to configure
/// the coroutine runtime behavior.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    /// `may`'s own coroutine stack **in `may` words** (what `may::config().set_stack_size` takes): `BRRTR_STACK_SIZE`
    /// bytes divided by the word size, default 256 KiB. Pass it to `set_stack_size` as is, or call [`Self::apply`];
    /// [`Self::stack_size_bytes`] gives bytes.
    pub stack_size: usize,
    /// Whether to cache JSON Schema validators (default: true)
    /// Eliminates per-request schema compilation overhead
    pub schema_cache_enabled: bool,
    /// May scheduler worker threads (`may::config().set_workers`). Minimum 2 when applied.
    pub may_workers: usize,
    /// Finished coroutines `may` keeps for reuse (`may::config().set_pool_capacity`), from
    /// `BRRTR_COROUTINE_POOL_CAPACITY` (default 100). A pooled coroutine keeps the stack pages it touched: at may's
    /// default of 1000, an idle service held ~1000 stacks, ~37 MiB (identity-session, 24 Oct 2026), and 1,042 stack
    /// mappings per Sesame pod in pp.
    pub coroutine_pool_capacity: usize,
    /// Idle `may` worker poll timeout in ms (`BRRTR_MAY_POLL_TIMEOUT_MS`, default 50; `may`'s default is 10).
    pub may_poll_timeout_ms: u64,
}

impl RuntimeConfig {
    /// Load configuration from environment variables.
    ///
    /// **IMPORTANT**: Stack sizes are made odd (if even) to enable May's internal
    /// stack usage tracking. This allows us to measure actual stack usage, not just
    /// allocation size.
    pub fn from_env() -> Self {
        let stack_bytes = match env::var("BRRTR_STACK_SIZE") {
            Ok(val) => crate::handler_stack::parse_size(&val)
                .filter(|&b| b > 0)
                .unwrap_or_else(|| {
                    eprintln!(
                        "[runtime][warn] BRRTR_STACK_SIZE={val:?} is not a size in bytes; using {DEFAULT_STACK_BYTES}"
                    );
                    DEFAULT_STACK_BYTES
                }),
            Err(_) => DEFAULT_STACK_BYTES,
        };
        let mut stack_size = crate::handler_stack::words(stack_bytes);

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
                "[telemetry] Adjusted stack size to {stack_size} words (odd) to enable usage tracking"
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

        let coroutine_pool_capacity = coroutine_pool_capacity_from_env();
        // Every service calls this before it spawns anything: take the pool default here too.
        ensure_coroutine_pool_capacity();
        let may_poll_timeout_ms = may_poll_timeout_ms_from_env();
        ensure_may_poll_timeout();

        RuntimeConfig {
            stack_size,
            schema_cache_enabled,
            may_workers,
            coroutine_pool_capacity,
            may_poll_timeout_ms,
        }
    }

    /// `may`'s coroutine stack in bytes.
    pub fn stack_size_bytes(&self) -> usize {
        self.stack_size.saturating_mul(std::mem::size_of::<usize>())
    }

    /// Configure `may` from this: coroutine stack size, scheduler workers and coroutine pool capacity. Call it before
    /// the first coroutine is spawned (`may` reads the workers and pool capacity when its scheduler starts).
    pub fn apply(&self) {
        may::config()
            .set_stack_size(self.stack_size)
            .set_workers(self.may_workers)
            .set_pool_capacity(self.coroutine_pool_capacity);
        // The poll timeout follows `ensure_may_poll_timeout`: a value the service set itself stays
        // unless BRRTR_MAY_POLL_TIMEOUT_MS asks for one explicitly.
        if env::var_os("BRRTR_MAY_POLL_TIMEOUT_MS").is_some()
            || may::config().get_timeout_ns() == MAY_DEFAULT_POLL_TIMEOUT_NS
        {
            may::config().set_timeout_ns(self.may_poll_timeout_ms.saturating_mul(1_000_000));
        }
    }
}

/// `may`'s coroutine stack when `BRRTR_STACK_SIZE` is unset: 256 KiB.
pub const DEFAULT_STACK_BYTES: usize = 256 * 1024;

/// BRRTRouter's coroutine pool capacity when `BRRTR_COROUTINE_POOL_CAPACITY` is unset.
pub const DEFAULT_COROUTINE_POOL_CAPACITY: usize = 100;

/// `may`'s built-in pool capacity (`may::config` `DEFAULT_POOL_CAPACITY`): what `get_pool_capacity` answers when no
/// one has set it.
const MAY_DEFAULT_POOL_CAPACITY: usize = 1000;

fn coroutine_pool_capacity_from_env() -> usize {
    env::var("BRRTR_COROUTINE_POOL_CAPACITY")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(DEFAULT_COROUTINE_POOL_CAPACITY)
}

/// Give `may` BRRTRouter's coroutine pool capacity (`BRRTR_COROUTINE_POOL_CAPACITY`, default 100) unless the service
/// already chose one. Idempotent; BRRTRouter calls it from [`RuntimeConfig::from_env`], `Dispatcher::new`, the handler
/// spawners and `HttpServer::start`, i.e. before its first coroutine, because `may` allocates the whole pool when its
/// scheduler starts. Unset, that is `may`'s 1,000 coroutines, each keeping the stack pages it last touched.
///
/// "Already chose" means `get_pool_capacity()` differs from `may`'s default of 1,000; a service that wants exactly
/// 1,000 sets `BRRTR_COROUTINE_POOL_CAPACITY=1000`.
pub fn ensure_coroutine_pool_capacity() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if may::config().get_pool_capacity() == MAY_DEFAULT_POOL_CAPACITY {
            may::config().set_pool_capacity(coroutine_pool_capacity_from_env());
        }
    });
}

/// BRRTRouter's idle `may` worker poll timeout when `BRRTR_MAY_POLL_TIMEOUT_MS` is unset.
pub const DEFAULT_MAY_POLL_TIMEOUT_MS: u64 = 50;

/// `may`'s built-in poll timeout (`may::config` `POLL_TIMEOUT_NS`, 10 ms).
const MAY_DEFAULT_POLL_TIMEOUT_NS: u64 = 10_000_000;

fn may_poll_timeout_ms_from_env() -> u64 {
    env::var("BRRTR_MAY_POLL_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(DEFAULT_MAY_POLL_TIMEOUT_MS)
}

/// Give `may` BRRTRouter's idle worker poll timeout (`BRRTR_MAY_POLL_TIMEOUT_MS`, default 50 ms). Without the variable,
/// a value the service already chose (`get_timeout_ns()` differs from `may`'s 10 ms) is kept. Idempotent. Each
/// worker reads it when its event loop starts, i.e. with the scheduler, so it must run before the first
/// coroutine: [`RuntimeConfig::from_env`] and `HttpServer::start` call it.
pub fn ensure_may_poll_timeout() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // An explicit BRRTR_MAY_POLL_TIMEOUT_MS wins; unset, a value the service chose stays.
        if env::var_os("BRRTR_MAY_POLL_TIMEOUT_MS").is_some()
            || may::config().get_timeout_ns() == MAY_DEFAULT_POLL_TIMEOUT_NS
        {
            may::config().set_timeout_ns(may_poll_timeout_ms_from_env() * 1_000_000);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test touches the process-wide env and may config, so the assertions run in sequence.
    #[test]
    fn stack_size_is_bytes_and_the_pool_defaults_to_100() {
        let _guard = crate::handler_stack::STACK_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let word = std::mem::size_of::<usize>();
        // SAFETY (test): env access serialized by STACK_ENV_LOCK.

        unsafe { std::env::remove_var("BRRTR_STACK_SIZE") };
        unsafe { std::env::remove_var("BRRTR_TRACK_STACK_USAGE") };
        unsafe { std::env::remove_var("BRRTR_COROUTINE_POOL_CAPACITY") };
        let c = RuntimeConfig::from_env();
        assert_eq!(c.stack_size_bytes(), DEFAULT_STACK_BYTES);
        assert_eq!(c.stack_size, DEFAULT_STACK_BYTES / word, "may takes words");
        assert_eq!(c.coroutine_pool_capacity, DEFAULT_COROUTINE_POOL_CAPACITY);
        assert_ne!(
            may::config().get_pool_capacity(),
            MAY_DEFAULT_POOL_CAPACITY,
            "from_env gives may BRRTRouter's pool capacity"
        );

        for (val, bytes) in [
            ("0x40000", 0x40000),
            ("2MiB", 2 << 20),
            ("524288", 524_288),
            ("256KiB", 256 << 10),
        ] {
            unsafe { std::env::set_var("BRRTR_STACK_SIZE", val) };
            let c = RuntimeConfig::from_env();
            assert_eq!(c.stack_size_bytes(), bytes, "{val}");
            assert_eq!(c.stack_size, bytes / word, "{val}");
        }
        unsafe { std::env::set_var("BRRTR_STACK_SIZE", "lots") };
        assert_eq!(
            RuntimeConfig::from_env().stack_size_bytes(),
            DEFAULT_STACK_BYTES
        );
        unsafe { std::env::remove_var("BRRTR_STACK_SIZE") };

        unsafe { std::env::remove_var("BRRTR_MAY_POLL_TIMEOUT_MS") };
        assert_eq!(
            RuntimeConfig::from_env().may_poll_timeout_ms,
            DEFAULT_MAY_POLL_TIMEOUT_MS
        );
        assert_ne!(
            may::config().get_timeout_ns(),
            MAY_DEFAULT_POLL_TIMEOUT_NS,
            "from_env gives may BRRTRouter's poll timeout"
        );
        for (val, ms) in [("250", 250), ("0", 50), ("soon", 50)] {
            unsafe { std::env::set_var("BRRTR_MAY_POLL_TIMEOUT_MS", val) };
            assert_eq!(RuntimeConfig::from_env().may_poll_timeout_ms, ms, "{val}");
        }
        unsafe { std::env::remove_var("BRRTR_MAY_POLL_TIMEOUT_MS") };

        // apply() keeps a timeout the service chose unless the env var asks for one.
        may::config().set_timeout_ns(7_000_000);
        RuntimeConfig::from_env().apply();
        assert_eq!(
            may::config().get_timeout_ns(),
            7_000_000,
            "service's own value kept"
        );
        unsafe { std::env::set_var("BRRTR_MAY_POLL_TIMEOUT_MS", "80") };
        RuntimeConfig::from_env().apply();
        assert_eq!(
            may::config().get_timeout_ns(),
            80_000_000,
            "explicit env wins"
        );
        unsafe { std::env::remove_var("BRRTR_MAY_POLL_TIMEOUT_MS") };
        may::config().set_timeout_ns(DEFAULT_MAY_POLL_TIMEOUT_MS * 1_000_000);
    }
}
