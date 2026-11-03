//! How long a request waits to be handled, and how late the coroutine scheduler runs.
//!
//! Rendered on `/metrics`:
//!
//! - `brrtrouter_request_queue_seconds{handler}` (histogram): from the dispatcher queueing a request
//!   to its handler coroutine (or worker pool) taking it. It grows when a handler's single
//!   coroutine is busy or its workers are all occupied.
//! - `brrtrouter_scheduler_lag_seconds` (histogram): a probe coroutine sleeps
//!   `BRRTR_SCHEDULER_PROBE_MS` (default 100) at a time and records how much later than asked it
//!   woke. Lateness is time every coroutine on the runtime waited, including the connection
//!   coroutines that read requests before they reach the router: workers held by blocking code,
//!   CPU throttling, or more ready coroutines than workers. `BRRTR_SCHEDULER_PROBE_MS=0` turns the
//!   probe off.
//!
//! Neither depends on request content; both are cheap enough to stay on (one histogram update per
//! request, one wake-up per probe interval).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Once, OnceLock, RwLock};
use std::time::{Duration, Instant};

use crate::dispatcher::HandlerRequest;

/// Upper bounds (seconds) of the histogram buckets, then `+Inf`.
const BUCKETS: [f64; 16] = [
    0.000_1, 0.000_5, 0.001, 0.002_5, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
    10.0, 30.0,
];

/// Most handler series kept; later handlers share the `_OTHER` series.
const MAX_HANDLERS: usize = 1024;
const OVERFLOW_HANDLER: &str = "_OTHER";

const DEFAULT_PROBE_MS: u64 = 100;

struct Histogram {
    /// Cumulative counts per bucket in `BUCKETS`, then `+Inf`.
    buckets: [AtomicU64; BUCKETS.len() + 1],
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
        let first = BUCKETS
            .iter()
            .position(|&le| secs <= le)
            .unwrap_or(BUCKETS.len());
        for bucket in &self.buckets[first..] {
            bucket.fetch_add(1, Ordering::Relaxed);
        }
        self.sum_nanos.fetch_add(
            u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn render(&self, out: &mut String, name: &str, labels: &str) {
        let sep = if labels.is_empty() { "" } else { "," };
        for (i, le) in BUCKETS.iter().enumerate() {
            let _ = writeln!(
                out,
                "{name}_bucket{{{labels}{sep}le=\"{le}\"}} {}",
                self.buckets[i].load(Ordering::Relaxed)
            );
        }
        let _ = writeln!(
            out,
            "{name}_bucket{{{labels}{sep}le=\"+Inf\"}} {}",
            self.buckets[BUCKETS.len()].load(Ordering::Relaxed)
        );
        let braces = if labels.is_empty() {
            String::new()
        } else {
            format!("{{{labels}}}")
        };
        #[allow(clippy::cast_precision_loss)]
        let sum = self.sum_nanos.load(Ordering::Relaxed) as f64 / 1e9;
        let _ = writeln!(out, "{name}_sum{braces} {sum}");
        let _ = writeln!(
            out,
            "{name}_count{braces} {}",
            self.count.load(Ordering::Relaxed)
        );
    }
}

fn queues() -> &'static RwLock<HashMap<String, Arc<Histogram>>> {
    static QUEUES: OnceLock<RwLock<HashMap<String, Arc<Histogram>>>> = OnceLock::new();
    QUEUES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn lag() -> &'static Histogram {
    static LAG: OnceLock<Histogram> = OnceLock::new();
    LAG.get_or_init(Histogram::new)
}

/// Record that `handler` took a request that had waited `waited` since it was queued.
pub fn observe_queue(handler: &str, waited: Duration) {
    if let Some(h) = queues().read().ok().and_then(|q| q.get(handler).cloned()) {
        h.observe(waited);
        return;
    }
    let Ok(mut q) = queues().write() else {
        return;
    };
    let key = if q.len() >= MAX_HANDLERS && !q.contains_key(handler) {
        OVERFLOW_HANDLER
    } else {
        handler
    };
    q.entry(key.to_string())
        .or_insert_with(|| Arc::new(Histogram::new()))
        .observe(waited);
}

/// Called by a handler coroutine or worker as it takes `req` off its queue.
pub fn dequeued(req: &HandlerRequest) {
    if let Some(guard) = &req.queue_guard {
        observe_queue(&req.handler_name, guard.queued_for());
    }
}

/// Record one probe wake-up that came `late` after the time asked for.
pub fn observe_scheduler_lag(late: Duration) {
    lag().observe(late);
}

fn probe_interval() -> Option<Duration> {
    let ms = std::env::var("BRRTR_SCHEDULER_PROBE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_PROBE_MS);
    (ms > 0).then(|| Duration::from_millis(ms))
}

/// Start the scheduler-lag probe coroutine (once per process; later calls do nothing).
pub fn start_scheduler_probe() {
    static START: Once = Once::new();
    START.call_once(|| {
        let Some(interval) = probe_interval() else {
            return;
        };
        // SAFETY: the coroutine owns everything it uses (a copied `Duration` and statics) and
        // never touches thread-local storage, which is what makes may's `spawn` unsafe.
        let spawned = unsafe {
            may::coroutine::Builder::new()
                .name("brrtrouter-scheduler-probe".to_string())
                .spawn(move || loop {
                    let asked = Instant::now();
                    may::coroutine::sleep(interval);
                    observe_scheduler_lag(asked.elapsed().saturating_sub(interval));
                })
        };
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "scheduler-lag probe not started");
        }
    });
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Append both histograms in Prometheus text format.
pub fn render_prometheus(out: &mut String) {
    out.push_str(
        "# HELP brrtrouter_request_queue_seconds Time from dispatch until the handler took the request.\n",
    );
    out.push_str("# TYPE brrtrouter_request_queue_seconds histogram\n");
    if let Ok(q) = queues().read() {
        let mut handlers: Vec<_> = q.iter().collect();
        handlers.sort_by(|a, b| a.0.cmp(b.0));
        for (handler, h) in handlers {
            h.render(
                out,
                "brrtrouter_request_queue_seconds",
                &format!("handler=\"{}\"", escape(handler)),
            );
        }
    }
    out.push_str(
        "# HELP brrtrouter_scheduler_lag_seconds How much later than asked the probe coroutine woke.\n",
    );
    out.push_str("# TYPE brrtrouter_scheduler_lag_seconds histogram\n");
    lag().render(out, "brrtrouter_scheduler_lag_seconds", "");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered() -> String {
        let mut out = String::new();
        render_prometheus(&mut out);
        out
    }

    #[test]
    fn queue_time_is_bucketed_per_handler() {
        observe_queue("queue_metrics_test_a", Duration::from_millis(3));
        observe_queue("queue_metrics_test_a", Duration::from_millis(300));
        let out = rendered();
        assert!(out.contains(
            "brrtrouter_request_queue_seconds_bucket{handler=\"queue_metrics_test_a\",le=\"0.0025\"} 0"
        ));
        assert!(out.contains(
            "brrtrouter_request_queue_seconds_bucket{handler=\"queue_metrics_test_a\",le=\"0.005\"} 1"
        ));
        assert!(out.contains(
            "brrtrouter_request_queue_seconds_bucket{handler=\"queue_metrics_test_a\",le=\"+Inf\"} 2"
        ));
        assert!(out.contains(
            "brrtrouter_request_queue_seconds_count{handler=\"queue_metrics_test_a\"} 2"
        ));
    }

    #[test]
    fn a_request_with_a_guard_records_its_wait_when_taken() {
        let (reply_tx, _rx) = may::sync::mpsc::channel();
        let mut req = HandlerRequest {
            request_id: crate::ids::RequestId::new(),
            method: http::Method::GET,
            path: "/x".to_string(),
            handler_name: "queue_metrics_test_b".to_string(),
            path_params: crate::router::ParamVec::new(),
            query_params: crate::router::ParamVec::new(),
            raw_query: None,
            headers: crate::dispatcher::HeaderVec::new(),
            cookies: crate::dispatcher::HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx,
            queue_guard: None,
            span: may_tracing::current(),
        };
        dequeued(&req); // no guard: nothing recorded
        assert!(!rendered().contains("handler=\"queue_metrics_test_b\""));
        req.queue_guard = Some(Arc::new(crate::dispatcher::QueueDepthGuard::timing_only()));
        std::thread::sleep(Duration::from_millis(15));
        dequeued(&req);
        let out = rendered();
        assert!(out.contains(
            "brrtrouter_request_queue_seconds_bucket{handler=\"queue_metrics_test_b\",le=\"0.01\"} 0"
        ));
        assert!(out.contains(
            "brrtrouter_request_queue_seconds_count{handler=\"queue_metrics_test_b\"} 1"
        ));
    }

    #[test]
    fn the_depth_guard_counts_until_dropped() {
        let depth = Arc::new(std::sync::atomic::AtomicUsize::new(1));
        let guard = crate::dispatcher::QueueDepthGuard::new(Arc::clone(&depth));
        assert_eq!(depth.load(Ordering::Relaxed), 1);
        drop(guard);
        assert_eq!(depth.load(Ordering::Relaxed), 0);
        drop(crate::dispatcher::QueueDepthGuard::timing_only()); // nothing to decrement
    }

    #[test]
    fn the_probe_records_wake_ups() {
        start_scheduler_probe();
        start_scheduler_probe(); // once
        std::thread::sleep(Duration::from_millis(450));
        let out = rendered();
        let count: u64 = out
            .lines()
            .find_map(|l| l.strip_prefix("brrtrouter_scheduler_lag_seconds_count "))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        assert!(count >= 2, "probe wake-ups recorded: {count}\n{out}");
    }
}
