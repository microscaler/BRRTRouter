//! Debug-session markers for freeze / regression investigations.
//!
//! ## Primary path: `tracing` → your log pipeline (Loki, Grafana, OTLP)
//!
//! Events use **target** `brrtrouter::debug_session` and message `cursor_debug_session` with structured
//! fields: `session_id`, `hypothesis_id`, `location`, `phase`, `data` (JSON string).
//!
//! **LogQL (Loki) examples**
//! ```text
//! {service_name="hauliage_consignments"} |= "cursor_debug_session"
//! {service_name="hauliage_consignments"} | json | hypothesis_id="H1_dispatch"
//! ```
//!
//! Enable via **ConfigMap / env** (merged automatically by [`crate::otel::init_logging_with_config`]
//! when **`BRRTR_DEBUG_SESSION`** is `1`, `true`, `yes`, or `on`):
//! ```text
//! BRRTR_DEBUG_SESSION=true
//! ```
//!
//! Or set the filter explicitly:
//! ```text
//! RUST_LOG=info,brrtrouter::debug_session=info
//! ```
//! ([`LOG_DIRECTIVE`] is exactly `brrtrouter::debug_session=info`.)
//!
//! ## Optional: NDJSON file (local Cursor ingest or ad-hoc copy)
//!
//! Set **`CURSOR_DEBUG_FILE=1`** to also append one NDJSON line per event to the path from
//! [`ndjson_log_path`]. **`CURSOR_DEBUG_MIRROR_STDERR=1`** duplicates the NDJSON line to stderr
//! (`kubectl logs`).

#![allow(clippy::inefficient_to_string)]
#![allow(clippy::format_push_string)]

use std::io::Write;
use std::path::PathBuf;

const SESSION_ID: &str = "92f9ed";
const RUN_ID: &str = "freeze-debug-1";
/// `tracing` target for [`ndjson`].
pub const TRACING_TARGET: &str = "brrtrouter::debug_session";

/// Full [`EnvFilter`] / `RUST_LOG` directive for debug-session events at INFO.
pub const LOG_DIRECTIVE: &str = "brrtrouter::debug_session=info";

/// Where NDJSON is written when **`CURSOR_DEBUG_FILE=1`**.
#[must_use]
pub fn ndjson_log_path() -> PathBuf {
    if let Ok(p) = std::env::var("CURSOR_DEBUG_LOG_PATH") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(format!(
                "{}/.cursor/debug-logs/debug-{}.log",
                home, SESSION_ID
            ));
        }
    }
    PathBuf::from(format!("/tmp/cursor-debug-{}.log", SESSION_ID))
}

fn file_mirror_enabled() -> bool {
    matches!(std::env::var("CURSOR_DEBUG_FILE").as_deref(), Ok("1"))
}

fn mirror_stderr_enabled() -> bool {
    matches!(
        std::env::var("CURSOR_DEBUG_MIRROR_STDERR").as_deref(),
        Ok("1" | "true" | "yes" | "TRUE" | "YES")
    )
}

fn open_log_append(path: &std::path::Path) -> Option<std::fs::File> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

/// Emit a structured debug event: **tracing** always; file/stderr optional.
pub fn ndjson(
    hypothesis_id: &'static str,
    location: &'static str,
    phase: &'static str,
    data: serde_json::Value,
) {
    // #region agent log
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());

    tracing::info!(
        target: TRACING_TARGET,
        session_id = SESSION_ID,
        hypothesis_id = hypothesis_id,
        location = location,
        phase = phase,
        data = %data_str,
        timestamp_ms = ts,
        run_id = RUN_ID,
        "cursor_debug_session"
    );

    if file_mirror_enabled() {
        let line = serde_json::json!({
            "sessionId": SESSION_ID,
            "hypothesisId": hypothesis_id,
            "location": location,
            "message": phase,
            "data": data,
            "timestamp": ts,
            "runId": RUN_ID,
        });
        let line_str = serde_json::to_string(&line).unwrap_or_else(|_| "{}".to_string());
        let path = ndjson_log_path();
        if let Some(mut f) = open_log_append(&path) {
            let _ = writeln!(f, "{line_str}");
        }
        if mirror_stderr_enabled() {
            let mut e = std::io::stderr().lock();
            let _ = writeln!(e, "cursor_debug_ndjson {line_str}");
        }
    }
    // #endregion
}
