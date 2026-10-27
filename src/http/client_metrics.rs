//! Client-side metrics for the BFF proxy: what a BFF asked of each downstream service and what came back.
//!
//! A slow screen used to show at the BFF (`http_server_request_duration_seconds` on the BFF's route) and at the
//! service separately, with nothing joining the two. Every [`super::proxy_untyped`] and [`super::proxy_sse`] call is
//! recorded here and rendered on `/metrics` beside the server metrics, under the OpenTelemetry HTTP client semantic
//! convention name:
//!
//! - `http_client_request_duration_seconds{server_address,http_request_method,http_response_status_code,error_type}`
//!   (histogram, the semantic-convention buckets). `server_address` is the downstream Kubernetes Service (`x-service`).
//!   `http_response_status_code` is what the downstream answered; it is absent when it never answered, and
//!   `error_type` then says why (`connect`, `timeout`, `dns`, ... as in the proxy's `reason`). A 4xx/5xx answer
//!   carries its status code as `error_type` too, as the convention asks.
//!
//! For an event stream the duration is the time to the downstream's response headers, not the stream's life.
//!
//! Nothing is rendered until the first proxied call, so services that proxy nothing export nothing new.
//!
//! The keep-alive pools behind `fetch_*` and the proxy are rendered too, once each pool has started
//! (`pool` is `fetch` or `proxy`), all counters since process start:
//!
//! - `brrtrouter_http_client_pool_hits_total{pool}`: requests sent on a reused connection
//! - `brrtrouter_http_client_pool_misses_total{pool}`: connections opened
//! - `brrtrouter_http_client_pool_evictions_total{pool,reason}`: connections closed instead of
//!   reused. `reason="expired"`: idle past the idle timeout or past the lifetime;
//!   `reason="discarded"`: found closed by the server, left unusable by its response, or failed.
//! - `brrtrouter_http_client_pool_waits_total{pool}`: requests that waited for a connection
//!   because the per-host or total limit was reached
//! - `brrtrouter_http_client_pool_stale_retries_total{pool}`: idempotent requests retried once on
//!   a new connection after a reused one failed

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::time::Duration;

/// OpenTelemetry HTTP semantic-convention buckets (seconds), as for the server histogram.
const BUCKETS: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    server: String,
    method: String,
    status: Option<u16>,
    error: Option<String>,
}

#[derive(Default)]
struct Hist {
    buckets: [u64; BUCKETS.len()],
    sum: f64,
    count: u64,
}

static CALLS: Mutex<BTreeMap<Key, Hist>> = Mutex::new(BTreeMap::new());

/// Record one downstream call. `status`: what the downstream answered (None when it never did); `error`: why not
/// (the proxy's reason code) when it did not.
pub fn record(
    server: &str,
    method: &str,
    status: Option<u16>,
    error: Option<&str>,
    elapsed: Duration,
) {
    let error = match (status, error) {
        (_, Some(e)) => Some(e.to_string()),
        (Some(s), None) if s >= 400 => Some(s.to_string()),
        _ => None,
    };
    let key = Key {
        server: server.to_string(),
        method: method.to_ascii_uppercase(),
        status,
        error,
    };
    let secs = elapsed.as_secs_f64();
    let mut calls = CALLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = calls.entry(key).or_default();
    for (i, b) in BUCKETS.iter().enumerate() {
        if secs <= *b {
            h.buckets[i] += 1;
        }
    }
    h.sum += secs;
    h.count += 1;
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

/// Append the client histogram and the pool counters in Prometheus text format (nothing before
/// the first call / before a pool starts).
pub fn render_prometheus(body: &mut String) {
    render_calls(body);
    let pools: Vec<(&str, may_minihttp::client::ClientStats)> = [
        ("fetch", super::fetch::fetch_pool_if_started()),
        ("proxy", super::proxy::proxy_pool_if_started()),
    ]
    .into_iter()
    .filter_map(|(name, pool)| pool.map(|p| (name, p.stats())))
    .collect();
    render_pools(body, &pools);
}

fn render_pools(body: &mut String, pools: &[(&str, may_minihttp::client::ClientStats)]) {
    if pools.is_empty() {
        return;
    }
    type Counter = (
        &'static str,
        &'static str,
        fn(&may_minihttp::client::ClientStats) -> u64,
    );
    const COUNTERS: [Counter; 4] = [
        (
            "brrtrouter_http_client_pool_hits_total",
            "Outbound requests sent on a reused keep-alive connection",
            |s| s.connections_reused,
        ),
        (
            "brrtrouter_http_client_pool_misses_total",
            "Outbound connections opened (no idle connection to reuse)",
            |s| s.connections_created,
        ),
        (
            "brrtrouter_http_client_pool_waits_total",
            "Outbound requests that waited for a connection at the pool limit",
            |s| s.pool_waits,
        ),
        (
            "brrtrouter_http_client_pool_stale_retries_total",
            "Idempotent outbound requests retried once after a reused connection failed",
            |s| s.stale_retries,
        ),
    ];
    for (name, help, value) in COUNTERS {
        let _ = writeln!(body, "# HELP {name} {help}");
        let _ = writeln!(body, "# TYPE {name} counter");
        for (pool, stats) in pools {
            let _ = writeln!(body, "{name}{{pool=\"{pool}\"}} {}", value(stats));
        }
    }
    body.push_str("# HELP brrtrouter_http_client_pool_evictions_total Outbound connections closed instead of reused\n");
    body.push_str("# TYPE brrtrouter_http_client_pool_evictions_total counter\n");
    for (pool, stats) in pools {
        let _ = writeln!(
            body,
            "brrtrouter_http_client_pool_evictions_total{{pool=\"{pool}\",reason=\"expired\"}} {}",
            stats.connections_expired
        );
        let _ = writeln!(
            body,
            "brrtrouter_http_client_pool_evictions_total{{pool=\"{pool}\",reason=\"discarded\"}} {}",
            stats.connections_discarded
        );
    }
}

fn render_calls(body: &mut String) {
    let calls = CALLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if calls.is_empty() {
        return;
    }
    body.push_str("# HELP http_client_request_duration_seconds Duration of the BFF's calls to downstream services\n");
    body.push_str("# TYPE http_client_request_duration_seconds histogram\n");
    for (k, h) in calls.iter() {
        let mut labels = format!(
            "server_address=\"{}\",http_request_method=\"{}\"",
            esc(&k.server),
            esc(&k.method)
        );
        if let Some(s) = k.status {
            let _ = write!(labels, ",http_response_status_code=\"{s}\"");
        }
        if let Some(e) = &k.error {
            let _ = write!(labels, ",error_type=\"{}\"", esc(e));
        }
        for (i, b) in BUCKETS.iter().enumerate() {
            let _ = writeln!(
                body,
                "http_client_request_duration_seconds_bucket{{{labels},le=\"{b}\"}} {}",
                h.buckets[i]
            );
        }
        let _ = writeln!(
            body,
            "http_client_request_duration_seconds_bucket{{{labels},le=\"+Inf\"}} {}",
            h.count
        );
        let _ = writeln!(
            body,
            "http_client_request_duration_seconds_sum{{{labels}}} {}",
            h.sum
        );
        let _ = writeln!(
            body,
            "http_client_request_duration_seconds_count{{{labels}}} {}",
            h.count
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pool_counters_per_pool() {
        let stats = may_minihttp::client::ClientStats {
            connections_created: 3,
            connections_reused: 40,
            connections_discarded: 2,
            connections_expired: 1,
            pool_waits: 5,
            stale_retries: 1,
            redirects_followed: 0,
        };
        let mut out = String::new();
        render_pools(&mut out, &[]);
        assert!(out.is_empty(), "nothing before a pool starts");
        render_pools(
            &mut out,
            &[
                ("fetch", stats),
                ("proxy", may_minihttp::client::ClientStats::default()),
            ],
        );
        assert!(out.contains("# TYPE brrtrouter_http_client_pool_hits_total counter"));
        assert!(out.contains("brrtrouter_http_client_pool_hits_total{pool=\"fetch\"} 40"));
        assert!(out.contains("brrtrouter_http_client_pool_misses_total{pool=\"fetch\"} 3"));
        assert!(out.contains("brrtrouter_http_client_pool_waits_total{pool=\"fetch\"} 5"));
        assert!(out.contains("brrtrouter_http_client_pool_stale_retries_total{pool=\"fetch\"} 1"));
        assert!(out.contains(
            "brrtrouter_http_client_pool_evictions_total{pool=\"fetch\",reason=\"expired\"} 1"
        ));
        assert!(out.contains(
            "brrtrouter_http_client_pool_evictions_total{pool=\"fetch\",reason=\"discarded\"} 2"
        ));
        assert!(out.contains("brrtrouter_http_client_pool_hits_total{pool=\"proxy\"} 0"));
    }

    #[test]
    fn records_status_errors_and_buckets() {
        record(
            "ph-test-market",
            "get",
            Some(200),
            None,
            Duration::from_millis(30),
        );
        record(
            "ph-test-market",
            "GET",
            Some(200),
            None,
            Duration::from_millis(300),
        );
        record(
            "ph-test-market",
            "GET",
            Some(503),
            None,
            Duration::from_millis(1),
        );
        record(
            "ph-test-orders",
            "POST",
            None,
            Some("connect"),
            Duration::from_millis(2),
        );
        let mut out = String::new();
        render_prometheus(&mut out);
        assert!(out.contains("# TYPE http_client_request_duration_seconds histogram"));
        let ok = "server_address=\"ph-test-market\",http_request_method=\"GET\",http_response_status_code=\"200\"";
        assert!(out.contains(&format!(
            "http_client_request_duration_seconds_count{{{ok}}} 2"
        )));
        assert!(out.contains(&format!(
            "http_client_request_duration_seconds_bucket{{{ok},le=\"0.05\"}} 1"
        )));
        assert!(out.contains(&format!(
            "http_client_request_duration_seconds_bucket{{{ok},le=\"0.5\"}} 2"
        )));
        assert!(out.contains(&format!(
            "http_client_request_duration_seconds_bucket{{{ok},le=\"+Inf\"}} 2"
        )));
        assert!(out.contains(
            "http_client_request_duration_seconds_count{server_address=\"ph-test-market\",http_request_method=\"GET\",http_response_status_code=\"503\",error_type=\"503\"} 1"
        ));
        assert!(out.contains(
            "http_client_request_duration_seconds_count{server_address=\"ph-test-orders\",http_request_method=\"POST\",error_type=\"connect\"} 1"
        ));
    }
}
