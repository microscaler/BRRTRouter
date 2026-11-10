//! `Server-Timing` on responses (W3C Server Timing), so a browser can split a request into server and network
//! time with the Resource Timing API.
//!
//! * `total;dur=<ms>`: on every response that is not streamed, the service's whole time on the request (parse,
//!   route, auth, validation, handler, response build). Responses that never reach a handler carry only this.
//! * `app;dur=<ms>`: the handler's own time, from dispatch to the response being built.
//! * `upstream;dur=<ms>`: on a proxied route, the time spent on the downstream call (for an event stream, to the
//!   downstream's response head). The service's own work is `app - upstream`.
//!
//! On by default; `BRRTR_SERVER_TIMING=off` (or `0`, `false`) turns it off. The CORS middleware exposes the header
//! (`Access-Control-Expose-Headers`) and sends `Timing-Allow-Origin` for an allowed origin, so cross-origin callers
//! can read both the header and the detailed timing phases.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Environment switch for the header.
pub const SERVER_TIMING_ENV: &str = "BRRTR_SERVER_TIMING";

/// The response header name (lower case, as BRRTRouter writes header names).
pub const HEADER: &str = "server-timing";

fn parse_enabled(raw: Option<&str>) -> bool {
    !matches!(
        raw.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("off" | "0" | "false" | "no")
    )
}

/// Whether responses carry `Server-Timing` (read once).
#[must_use]
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| parse_enabled(std::env::var(SERVER_TIMING_ENV).ok().as_deref()))
}

/// One metric entry, `name;dur=<ms>` with microsecond precision (`app;dur=12.345`).
#[must_use]
pub fn entry(name: &str, dur: Duration) -> String {
    format!("{name};dur={:.3}", dur.as_secs_f64() * 1000.0)
}

/// Add `name;dur=` to the response headers: appended to an existing `server-timing` header (a proxied response
/// already carries `upstream`), else as a new header.
pub fn append(headers: &mut crate::dispatcher::HeaderVec, name: &str, dur: Duration) {
    let e = entry(name, dur);
    if let Some((_, v)) = headers
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case(HEADER))
    {
        v.push_str(", ");
        v.push_str(&e);
    } else {
        headers.push((Arc::from(HEADER), e));
    }
}

/// `append` when the header is on.
pub fn record(headers: &mut crate::dispatcher::HeaderVec, name: &str, dur: Duration) {
    if enabled() {
        append(headers, name, dur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_unless_switched_off() {
        assert!(parse_enabled(None));
        assert!(parse_enabled(Some("on")));
        assert!(parse_enabled(Some("1")));
        for off in ["off", "OFF", "0", "false", " no "] {
            assert!(!parse_enabled(Some(off)), "{off}");
        }
    }

    #[test]
    fn entry_is_milliseconds() {
        assert_eq!(
            entry("app", Duration::from_micros(12_345)),
            "app;dur=12.345"
        );
        assert_eq!(entry("upstream", Duration::ZERO), "upstream;dur=0.000");
    }

    #[test]
    fn append_adds_or_extends_the_header() {
        let mut h = crate::dispatcher::HeaderVec::new();
        append(&mut h, "upstream", Duration::from_millis(4));
        append(&mut h, "app", Duration::from_millis(5));
        let v: Vec<_> = h.iter().filter(|(k, _)| k.as_ref() == HEADER).collect();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "upstream;dur=4.000, app;dur=5.000");
    }
}
