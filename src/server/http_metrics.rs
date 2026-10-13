//! HTTP server and process metrics under their industry-standard names, so a stock Prometheus scrape (Google Cloud
//! Managed Service for Prometheus, Grafana's HTTP dashboards, alert packs) works without BRRTRouter-specific mapping.
//!
//! - `http_server_request_duration_seconds` - OpenTelemetry `http.server.request.duration` as Prometheus names it:
//!   a histogram with the semantic-convention buckets and labels `http_request_method`, `http_route` (the matched
//!   template, omitted when nothing matched), `http_response_status_code`, and `error_type` on 5xx. Traffic, error
//!   rate and latency percentiles all derive from it (Cloud Monitoring's Application Monitoring golden signals).
//! - `http_server_active_requests` - OpenTelemetry `http.server.active_requests`, by `http_request_method`.
//! - `process_*` - the Prometheus client-library process collector names (Linux, from `/proc/self`).
//!
//! Every request is recorded once, when its request logger completes - including those refused before routing
//! (401, 404, CORS), which the dispatcher middleware never sees.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

/// OpenTelemetry semantic-convention advisory buckets for `http.server.request.duration` (seconds).
pub const DURATION_BUCKETS: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

/// Series beyond this many (method x route x status x error) fold into `http_route="_OTHER"`: routes are templates,
/// so this only trips on a pathological spec, never on raw paths.
const MAX_SERIES: usize = 4096;

/// Methods the semantic conventions name; anything else is `_OTHER`.
const KNOWN_METHODS: [&str; 9] = [
    "CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "TRACE",
];

fn method_label(method: &str) -> &'static str {
    KNOWN_METHODS
        .iter()
        .find(|m| m.eq_ignore_ascii_case(method))
        .copied()
        .unwrap_or("_OTHER")
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SeriesKey {
    method: &'static str,
    route: Option<String>,
    status: u16,
}

struct Histogram {
    /// Cumulative counts per bucket in `DURATION_BUCKETS`, then `+Inf`.
    buckets: [AtomicU64; DURATION_BUCKETS.len() + 1],
    sum_nanos: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    fn new() -> Self {
        Self {
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            sum_nanos: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn observe(&self, duration: Duration) {
        let secs = duration.as_secs_f64();
        let first = DURATION_BUCKETS
            .iter()
            .position(|&le| secs <= le)
            .unwrap_or(DURATION_BUCKETS.len());
        for bucket in &self.buckets[first..] {
            bucket.fetch_add(1, Ordering::Relaxed);
        }
        self.sum_nanos.fetch_add(
            u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

struct Registry {
    series: RwLock<HashMap<SeriesKey, Arc<Histogram>>>,
    active: [AtomicI64; KNOWN_METHODS.len() + 1],
}

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Registry {
        series: RwLock::new(HashMap::new()),
        active: std::array::from_fn(|_| AtomicI64::new(0)),
    })
}

fn method_index(label: &'static str) -> usize {
    KNOWN_METHODS
        .iter()
        .position(|m| *m == label)
        .unwrap_or(KNOWN_METHODS.len())
}

/// Record one completed request.
pub fn observe(method: &str, route: Option<&str>, status: u16, duration: Duration) {
    let reg = registry();
    let mut key = SeriesKey {
        method: method_label(method),
        route: route.filter(|r| !r.is_empty()).map(str::to_string),
        status,
    };
    let existing = reg.series.read().ok().and_then(|s| s.get(&key).cloned());
    let histogram = match existing {
        Some(h) => h,
        None => {
            let Ok(mut series) = reg.series.write() else {
                return;
            };
            if !series.contains_key(&key) && series.len() >= MAX_SERIES {
                key.route = Some("_OTHER".to_string());
            }
            Arc::clone(
                series
                    .entry(key)
                    .or_insert_with(|| Arc::new(Histogram::new())),
            )
        }
    };
    histogram.observe(duration);
}

/// Counts a request as active until dropped (`http.server.active_requests`).
pub struct ActiveRequest(usize);

impl ActiveRequest {
    #[must_use]
    pub fn start(method: &str) -> Self {
        let index = method_index(method_label(method));
        registry().active[index].fetch_add(1, Ordering::Relaxed);
        Self(index)
    }
}

impl Drop for ActiveRequest {
    fn drop(&mut self) {
        registry().active[self.0].fetch_sub(1, Ordering::Relaxed);
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Prometheus text exposition of the HTTP server metrics.
pub fn render_http(out: &mut String) {
    let reg = registry();
    out.push_str("# HELP http_server_request_duration_seconds Duration of HTTP server requests.\n");
    out.push_str("# TYPE http_server_request_duration_seconds histogram\n");
    if let Ok(series) = reg.series.read() {
        let mut keys: Vec<&SeriesKey> = series.keys().collect();
        keys.sort_by(|a, b| {
            (a.route.as_deref(), a.method, a.status).cmp(&(b.route.as_deref(), b.method, b.status))
        });
        for key in keys {
            let h = &series[key];
            let mut labels = format!("http_request_method=\"{}\"", key.method);
            if let Some(route) = &key.route {
                let _ = write!(labels, ",http_route=\"{}\"", escape(route));
            }
            let _ = write!(labels, ",http_response_status_code=\"{}\"", key.status);
            if key.status >= 500 {
                let _ = write!(labels, ",error_type=\"{}\"", key.status);
            }
            for (i, le) in DURATION_BUCKETS.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "http_server_request_duration_seconds_bucket{{{labels},le=\"{le}\"}} {}",
                    h.buckets[i].load(Ordering::Relaxed)
                );
            }
            let _ = writeln!(
                out,
                "http_server_request_duration_seconds_bucket{{{labels},le=\"+Inf\"}} {}",
                h.buckets[DURATION_BUCKETS.len()].load(Ordering::Relaxed)
            );
            let sum = h.sum_nanos.load(Ordering::Relaxed) as f64 / 1e9;
            let _ = writeln!(
                out,
                "http_server_request_duration_seconds_sum{{{labels}}} {sum:.9}"
            );
            let _ = writeln!(
                out,
                "http_server_request_duration_seconds_count{{{labels}}} {}",
                h.count.load(Ordering::Relaxed)
            );
        }
    }
    out.push_str("# HELP http_server_active_requests Number of active HTTP server requests.\n");
    out.push_str("# TYPE http_server_active_requests gauge\n");
    for (i, method) in KNOWN_METHODS
        .iter()
        .chain(std::iter::once(&"_OTHER"))
        .enumerate()
    {
        let active = reg.active[i].load(Ordering::Relaxed);
        if active != 0 || *method == "GET" {
            let _ = writeln!(
                out,
                "http_server_active_requests{{http_request_method=\"{method}\"}} {active}"
            );
        }
    }
}

/// Prometheus client-library process collector metrics (Linux; nothing elsewhere).
pub fn render_process(out: &mut String) {
    #[cfg(target_os = "linux")]
    linux_process::render(out);
    #[cfg(not(target_os = "linux"))]
    let _ = out;
}

#[cfg(target_os = "linux")]
mod linux_process {
    use std::fmt::Write as _;

    /// USER_HZ: the unit of /proc/<pid>/stat CPU times, 100 on every mainstream Linux.
    const CLOCK_TICKS: f64 = 100.0;

    fn status_kib(status: &str, key: &str) -> Option<u64> {
        status
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
    }

    pub(super) fn render(out: &mut String) {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
        // Fields after the parenthesised command name (which may contain spaces).
        let fields: Vec<&str> = stat
            .rsplit_once(')')
            .map(|(_, rest)| rest.split_whitespace().collect())
            .unwrap_or_default();
        // man 5 proc: utime(14) stime(15) starttime(22), counted from field 3 (state) = index 0 here.
        let field = |n: usize| fields.get(n - 3).and_then(|v| v.parse::<f64>().ok());
        if let (Some(utime), Some(stime)) = (field(14), field(15)) {
            out.push_str("# HELP process_cpu_seconds_total Total user and system CPU time spent in seconds.\n");
            out.push_str("# TYPE process_cpu_seconds_total counter\n");
            let _ = writeln!(
                out,
                "process_cpu_seconds_total {}",
                (utime + stime) / CLOCK_TICKS
            );
        }
        let boot_time = std::fs::read_to_string("/proc/stat").ok().and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("btime "))
                .and_then(|v| v.trim().parse::<f64>().ok())
        });
        if let (Some(boot), Some(start)) = (boot_time, field(22)) {
            out.push_str("# HELP process_start_time_seconds Start time of the process since unix epoch in seconds.\n");
            out.push_str("# TYPE process_start_time_seconds gauge\n");
            let _ = writeln!(
                out,
                "process_start_time_seconds {}",
                boot + start / CLOCK_TICKS
            );
        }
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        if let Some(rss) = status_kib(&status, "VmRSS:") {
            out.push_str("# HELP process_resident_memory_bytes Resident memory size in bytes.\n");
            out.push_str("# TYPE process_resident_memory_bytes gauge\n");
            let _ = writeln!(out, "process_resident_memory_bytes {}", rss * 1024);
        }
        if let Some(vm) = status_kib(&status, "VmSize:") {
            out.push_str("# HELP process_virtual_memory_bytes Virtual memory size in bytes.\n");
            out.push_str("# TYPE process_virtual_memory_bytes gauge\n");
            let _ = writeln!(out, "process_virtual_memory_bytes {}", vm * 1024);
        }
        if let Ok(fds) = std::fs::read_dir("/proc/self/fd") {
            out.push_str("# HELP process_open_fds Number of open file descriptors.\n");
            out.push_str("# TYPE process_open_fds gauge\n");
            let _ = writeln!(out, "process_open_fds {}", fds.count());
        }
        let max_fds = std::fs::read_to_string("/proc/self/limits")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("Max open files"))
                    .and_then(|l| l.split_whitespace().nth(3))
                    .and_then(|v| v.parse::<u64>().ok())
            });
        if let Some(max) = max_fds {
            out.push_str("# HELP process_max_fds Maximum number of open file descriptors.\n");
            out.push_str("# TYPE process_max_fds gauge\n");
            let _ = writeln!(out, "process_max_fds {max}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render() -> String {
        let mut out = String::new();
        render_http(&mut out);
        out
    }

    #[test]
    fn a_request_lands_in_its_route_status_series_and_buckets() {
        observe(
            "GET",
            Some("/__test/orders/{id}"),
            200,
            Duration::from_millis(30),
        );
        observe(
            "get",
            Some("/__test/orders/{id}"),
            200,
            Duration::from_millis(300),
        );
        let out = render();
        let labels = "http_request_method=\"GET\",http_route=\"/__test/orders/{id}\",http_response_status_code=\"200\"";
        assert!(
            out.contains(&format!(
                "http_server_request_duration_seconds_count{{{labels}}} 2"
            )),
            "{out}"
        );
        assert!(out.contains(&format!(
            "http_server_request_duration_seconds_bucket{{{labels},le=\"0.025\"}} 0"
        )));
        assert!(out.contains(&format!(
            "http_server_request_duration_seconds_bucket{{{labels},le=\"0.05\"}} 1"
        )));
        assert!(out.contains(&format!(
            "http_server_request_duration_seconds_bucket{{{labels},le=\"0.5\"}} 2"
        )));
        assert!(out.contains(&format!(
            "http_server_request_duration_seconds_bucket{{{labels},le=\"+Inf\"}} 2"
        )));
    }

    #[test]
    fn server_errors_carry_error_type_and_unrouted_requests_no_route() {
        observe("POST", Some("/__test/fail"), 503, Duration::from_millis(1));
        observe("GET", None, 404, Duration::from_millis(1));
        let out = render();
        assert!(out.contains(
            "http_server_request_duration_seconds_count{http_request_method=\"POST\",http_route=\"/__test/fail\",http_response_status_code=\"503\",error_type=\"503\"} 1"
        ));
        assert!(out.contains("http_server_request_duration_seconds_count{http_request_method=\"GET\",http_response_status_code=\"404\"}"));
    }

    #[test]
    fn unknown_methods_are_other_and_active_requests_count_down() {
        assert_eq!(method_label("PROPFIND"), "_OTHER");
        let guard = ActiveRequest::start("PROPFIND");
        assert!(render().contains("http_server_active_requests{http_request_method=\"_OTHER\"} 1"));
        drop(guard);
        assert!(!render().contains("http_server_active_requests{http_request_method=\"_OTHER\"} 1"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_metrics_use_the_standard_names() {
        let mut out = String::new();
        render_process(&mut out);
        for name in [
            "process_cpu_seconds_total ",
            "process_resident_memory_bytes ",
            "process_virtual_memory_bytes ",
            "process_start_time_seconds ",
            "process_open_fds ",
            "process_max_fds ",
        ] {
            assert!(out.contains(name), "missing {name}: {out}");
        }
    }
}
