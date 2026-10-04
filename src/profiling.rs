//! Optional continuous profiling via a Pyroscope agent (CPU, pprof backend).
//!
//! Disabled by default — enabled at runtime by environment variables so
//! library consumers are unaffected unless they opt in:
//!
//! - `PYROSCOPE_SERVER_ADDRESS` (required to enable, e.g. `http://pyroscope:4040`)
//! - `PYROSCOPE_APPLICATION_NAME` (defaults to `brrtrouter-service`)
//! - `PYROSCOPE_SAMPLING_RATE` (defaults to `100` Hz)
//!
//! Started from [`crate::server::AppService::new`], so every generated service
//! gets profiling with zero code changes. The agent session is intentionally
//! leaked to keep profiling active for the lifetime of the process.

use std::sync::OnceLock;

use pyroscope::backend::{pprof_backend, BackendConfig, PprofConfig};
use pyroscope::pyroscope::PyroscopeAgentBuilder;

/// Start the Pyroscope agent if `PYROSCOPE_SERVER_ADDRESS` is set.
/// Idempotent — subsequent calls are no-ops.
pub fn init_from_env() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let Ok(url) = std::env::var("PYROSCOPE_SERVER_ADDRESS") else {
            return;
        };
        let app = std::env::var("PYROSCOPE_APPLICATION_NAME")
            .unwrap_or_else(|_| "brrtrouter-service".to_string());
        let rate: u32 = std::env::var("PYROSCOPE_SAMPLING_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        let agent = PyroscopeAgentBuilder::new(
            &url,
            &app,
            rate,
            "pyroscope-rs",
            env!("CARGO_PKG_VERSION"),
            pprof_backend(PprofConfig::default(), BackendConfig::default()),
        )
        .build()
        .and_then(|agent| agent.start());

        match agent {
            Ok(session) => {
                // Keep profiling active for the process lifetime.
                std::mem::forget(session);
                eprintln!("[profiling] pyroscope agent started: {url} app={app} rate={rate}Hz");
            }
            Err(e) => eprintln!("[profiling] failed to start pyroscope agent: {e}"),
        }
    });
}
