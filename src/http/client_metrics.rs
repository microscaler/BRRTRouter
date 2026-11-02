//! Client-side metrics: what this service asked of the services it calls and what came back.
//!
//! A slow screen used to show at the BFF (`http_server_request_duration_seconds` on the BFF's route) and at the
//! service separately, with nothing joining the two. Every [`super::proxy_untyped`] and [`super::proxy_sse`] call,
//! and every `fetch_*` call through the fetch pool, is recorded here and rendered on `/metrics` beside the server
//! metrics, under the OpenTelemetry HTTP client semantic convention name:
//!
//! - `http_client_request_duration_seconds{server_address,http_request_method,http_route,http_response_status_code,error_type}`
//!   (histogram, the semantic-convention buckets). `server_address` is the downstream Kubernetes Service (`x-service`)
//!   for a proxied call, the URL's host for a fetch. `http_route` is the downstream path template of a proxied call
//!   (`x-brrtrouter-downstream-path`); fetches have none. `http_response_status_code` is what the server answered; it
//!   is absent when it never answered, and `error_type` then says why (the proxy's `reason`: `connect`, `timeout`,
//!   `dns`, ...; for a fetch, the client's classification: `connection`, `timeout`, `dns`, `tls`, ...). A 4xx/5xx
//!   answer carries its status code as `error_type` too, as the convention asks.
//!
//! For an event stream the duration is the time to the downstream's response headers, not the stream's life.
//!
//! Nothing is rendered until the first recorded call, so services that call nothing export nothing new. Recording a
//! call allocates only the first time its label set is seen.
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

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher, Hash, Hasher};
use std::sync::Mutex;
use std::time::Duration;

/// OpenTelemetry HTTP semantic-convention buckets (seconds), as for the server histogram.
const BUCKETS: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

/// `error_type`: the status code itself for a 4xx/5xx answer (as the semantic convention asks),
/// or why there was no answer.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ErrorLabel {
    Status,
    Text(String),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    server: String,
    method: String,
    route: Option<String>,
    status: Option<u16>,
    error: Option<ErrorLabel>,
}

/// The labels of one call, borrowed: looking up an existing series allocates nothing.
struct Labels<'a> {
    server: &'a str,
    method: &'a str,
    route: Option<&'a str>,
    status: Option<u16>,
    error: Option<&'a str>,
}

impl Labels<'_> {
    fn error_is_status(&self) -> bool {
        self.error.is_none() && self.status.is_some_and(|s| s >= 400)
    }

    fn hash(&self) -> u64 {
        let mut h = BuildHasherDefault::<DefaultHasher>::default().build_hasher();
        self.server.hash(&mut h);
        for b in self.method.bytes() {
            b.to_ascii_uppercase().hash(&mut h);
        }
        self.route.hash(&mut h);
        self.status.hash(&mut h);
        self.error.hash(&mut h);
        self.error_is_status().hash(&mut h);
        h.finish()
    }

    fn matches(&self, key: &Key) -> bool {
        key.server == self.server
            && key.method.eq_ignore_ascii_case(self.method)
            && key.route.as_deref() == self.route
            && key.status == self.status
            && match (&key.error, self.error) {
                (None, None) => !self.error_is_status(),
                (Some(ErrorLabel::Status), None) => self.error_is_status(),
                (Some(ErrorLabel::Text(t)), Some(e)) => t == e,
                _ => false,
            }
    }

    fn to_key(&self) -> Key {
        Key {
            server: self.server.to_string(),
            method: self.method.to_ascii_uppercase(),
            route: self.route.map(str::to_string),
            status: self.status,
            error: match self.error {
                Some(e) => Some(ErrorLabel::Text(e.to_string())),
                None if self.error_is_status() => Some(ErrorLabel::Status),
                None => None,
            },
        }
    }
}

#[derive(Default)]
struct Hist {
    buckets: [u64; BUCKETS.len()],
    sum: f64,
    count: u64,
}

/// Series, indexed by the hash of their labels (collisions share a bucket list).
#[derive(Default)]
struct Calls {
    series: Vec<(Key, Hist)>,
    index: HashMap<u64, Vec<usize>>,
}

static CALLS: Mutex<Option<Calls>> = Mutex::new(None);

/// Record one outbound call. `route`: the path template of what was called, when known (never a
/// path with ids in it); `status`: what the server answered (None when it never did); `error`: why
/// not, when it did not. Allocates only the first time a label set is seen.
pub fn record(
    server: &str,
    method: &str,
    route: Option<&str>,
    status: Option<u16>,
    error: Option<&str>,
    elapsed: Duration,
) {
    let labels = Labels {
        server,
        method,
        route,
        status,
        error,
    };
    let hash = labels.hash();
    let secs = elapsed.as_secs_f64();
    let mut guard = CALLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let calls = guard.get_or_insert_with(Calls::default);
    let found = calls.index.get(&hash).and_then(|slots| {
        slots
            .iter()
            .copied()
            .find(|&i| labels.matches(&calls.series[i].0))
    });
    let slot = match found {
        Some(i) => i,
        None => {
            calls.series.push((labels.to_key(), Hist::default()));
            let i = calls.series.len() - 1;
            calls.index.entry(hash).or_default().push(i);
            i
        }
    };
    let h = &mut calls.series[slot].1;
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
    let guard = CALLS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(calls) = guard.as_ref().filter(|c| !c.series.is_empty()) else {
        return;
    };
    let mut series: Vec<&(Key, Hist)> = calls.series.iter().collect();
    series.sort_by(|a, b| a.0.cmp(&b.0));
    body.push_str("# HELP http_client_request_duration_seconds Duration of the BFF's calls to downstream services\n");
    body.push_str("# TYPE http_client_request_duration_seconds histogram\n");
    for (k, h) in series {
        let mut labels = format!(
            "server_address=\"{}\",http_request_method=\"{}\"",
            esc(&k.server),
            esc(&k.method)
        );
        if let Some(r) = &k.route {
            let _ = write!(labels, ",http_route=\"{}\"", esc(r));
        }
        if let Some(s) = k.status {
            let _ = write!(labels, ",http_response_status_code=\"{s}\"");
        }
        match &k.error {
            Some(ErrorLabel::Text(e)) => {
                let _ = write!(labels, ",error_type=\"{}\"", esc(e));
            }
            Some(ErrorLabel::Status) => {
                let _ = write!(labels, ",error_type=\"{}\"", k.status.unwrap_or_default());
            }
            None => {}
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
            None,
            Some(200),
            None,
            Duration::from_millis(30),
        );
        record(
            "ph-test-market",
            "GET",
            None,
            Some(200),
            None,
            Duration::from_millis(300),
        );
        record(
            "ph-test-market",
            "GET",
            None,
            Some(503),
            None,
            Duration::from_millis(1),
        );
        record(
            "ph-test-orders",
            "POST",
            Some("/orders/{id}"),
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
            "http_client_request_duration_seconds_count{server_address=\"ph-test-orders\",http_request_method=\"POST\",http_route=\"/orders/{id}\",error_type=\"connect\"} 1"
        ));
    }
}
