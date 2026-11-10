//! Relayed event streams at the proxy ([`super::proxy_sse`]): how many are open, how many opened and closed (and
//! why), how long they lived, and how many distinct principals hold one.
//!
//! At a BFF every browser `EventSource` on a proxied stream route is one relayed stream, so these are the live
//! clients as the edge sees them. Series, labelled `operation` (the route's spec `operationId`, so bounded by the
//! spec; past [`MAX_OPERATIONS`] distinct values they fold into `_other`, and an empty one is `_unmatched`):
//!
//! - `brrtrouter_sse_streams{operation}` gauge: relayed streams open now;
//! - `brrtrouter_sse_opened_total{operation}` counter;
//! - `brrtrouter_sse_closed_total{operation,reason}` counter, `reason` one of [`CloseReason`];
//! - `brrtrouter_sse_duration_seconds{operation}` histogram: how long a stream lived;
//! - `brrtrouter_sse_principals{operation}` gauge: distinct principals with a stream open now.
//!
//! Principals come only from `HandlerRequest::jwt_claims`, which the router fills after the route's security check
//! has passed; nothing here decodes a token. A stream without a verified subject is counted as a stream, not as a
//! principal. A principal is held as a 64-bit hash of its `sub` with a count of its open streams, so the gauge is a
//! set size; no subject is ever exported or used as a label.
//!
//! Everything is recorded at open and close only: relaying a frame costs nothing here. Each operation's handles are
//! resolved once and kept for the life of the process.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Distinct `operation` label values kept; the rest fold into `_other`.
pub const MAX_OPERATIONS: usize = 128;

/// Stream lifetimes, seconds: a reconnect loop (seconds) to a tab left open all day (hours).
const BUCKETS: [f64; 12] = [
    1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0, 7200.0, 14400.0, 28800.0,
];

/// Why a relayed stream ended (the `reason` label).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// The client went away (its connection closed).
    ClientClosed,
    /// The client read too slowly and its queue filled.
    SlowClient,
    /// The downstream ended the stream.
    DownstreamEnded,
    /// The downstream stayed silent past the relay's idle limit.
    IdleTimeout,
    /// Anything else: a read error, an oversized frame, a relay that never started.
    Error,
}

impl CloseReason {
    /// Every reason, in label order.
    pub const ALL: [CloseReason; 5] = [
        CloseReason::ClientClosed,
        CloseReason::SlowClient,
        CloseReason::DownstreamEnded,
        CloseReason::IdleTimeout,
        CloseReason::Error,
    ];

    /// The label value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            CloseReason::ClientClosed => "client_closed",
            CloseReason::SlowClient => "slow_client",
            CloseReason::DownstreamEnded => "downstream_ended",
            CloseReason::IdleTimeout => "idle_timeout",
            CloseReason::Error => "error",
        }
    }
}

struct Operation {
    open: AtomicI64,
    opened: AtomicU64,
    closed: [AtomicU64; 5],
    buckets: [AtomicU64; 12],
    count: AtomicU64,
    sum_micros: AtomicU64,
    /// Open streams per principal key; the gauge is the number of keys.
    principals: Mutex<HashMap<u64, u32>>,
}

impl Operation {
    fn new() -> Self {
        Self {
            open: AtomicI64::new(0),
            opened: AtomicU64::new(0),
            closed: Default::default(),
            buckets: Default::default(),
            count: AtomicU64::new(0),
            sum_micros: AtomicU64::new(0),
            principals: Mutex::new(HashMap::new()),
        }
    }

    fn principals(&self) -> usize {
        self.principals.lock().map_or(0, |p| p.len())
    }
}

/// Operations by label, in first-seen order. Leaked: one per operation, for the life of the process.
static OPERATIONS: Mutex<Vec<(String, &'static Operation)>> = Mutex::new(Vec::new());

fn operation(name: &str) -> &'static Operation {
    let mut ops = OPERATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    operation_in(&mut ops, name)
}

fn operation_in(ops: &mut Vec<(String, &'static Operation)>, name: &str) -> &'static Operation {
    let name = if name.is_empty() { "_unmatched" } else { name };
    if let Some((_, op)) = ops.iter().find(|(n, _)| n == name) {
        return op;
    }
    let label = if ops.len() >= MAX_OPERATIONS {
        "_other"
    } else {
        name
    };
    if let Some((_, op)) = ops.iter().find(|(n, _)| n == label) {
        return op;
    }
    let op: &'static Operation = Box::leak(Box::new(Operation::new()));
    ops.push((label.to_owned(), op));
    op
}

/// The key a principal is counted under: a hash of the verified `sub` claim, if there is one.
#[must_use]
pub fn principal_key(jwt_claims: Option<&serde_json::Value>) -> Option<u64> {
    let sub = jwt_claims?.get("sub")?.as_str()?;
    if sub.is_empty() {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    sub.hash(&mut h);
    Some(h.finish())
}

/// One open relayed stream. [`StreamGuard::close`] records why it ended; dropping it unclosed records
/// [`CloseReason::Error`] (a relay that never started, or a panic).
pub struct StreamGuard {
    op: &'static Operation,
    principal: Option<u64>,
    started: Instant,
    closed: bool,
}

/// A relayed stream opened for `operation` (the route's `operationId`), held by `principal` ([`principal_key`]).
#[must_use]
pub fn opened(operation_id: &str, principal: Option<u64>) -> StreamGuard {
    let op = operation(operation_id);
    op.opened.fetch_add(1, Ordering::Relaxed);
    op.open.fetch_add(1, Ordering::Relaxed);
    if let Some(p) = principal {
        if let Ok(mut m) = op.principals.lock() {
            *m.entry(p).or_insert(0) += 1;
        }
    }
    StreamGuard {
        op,
        principal,
        started: Instant::now(),
        closed: false,
    }
}

impl StreamGuard {
    /// The stream ended for `reason`.
    pub fn close(mut self, reason: CloseReason) {
        self.record(reason);
    }

    fn record(&mut self, reason: CloseReason) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        let op = self.op;
        op.open.fetch_sub(1, Ordering::Relaxed);
        op.closed[reason as usize].fetch_add(1, Ordering::Relaxed);
        if let Some(p) = self.principal {
            if let Ok(mut m) = op.principals.lock() {
                if let Some(n) = m.get_mut(&p) {
                    *n -= 1;
                    if *n == 0 {
                        m.remove(&p);
                    }
                }
            }
        }
        observe(op, self.started.elapsed());
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        self.record(CloseReason::Error);
    }
}

fn observe(op: &Operation, lived: Duration) {
    let secs = lived.as_secs_f64();
    for (i, b) in BUCKETS.iter().enumerate() {
        if secs <= *b {
            op.buckets[i].fetch_add(1, Ordering::Relaxed);
        }
    }
    op.count.fetch_add(1, Ordering::Relaxed);
    let micros = u64::try_from(lived.as_micros()).unwrap_or(u64::MAX);
    op.sum_micros.fetch_add(micros, Ordering::Relaxed);
}

fn escape(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Prometheus text for every operation that relayed a stream, appended to `/metrics`.
pub fn render_prometheus(body: &mut String) {
    let ops: Vec<(String, &'static Operation)> = OPERATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if ops.is_empty() {
        return;
    }
    let _ = writeln!(
        body,
        "# HELP brrtrouter_sse_streams Relayed event streams open now."
    );
    let _ = writeln!(body, "# TYPE brrtrouter_sse_streams gauge");
    for (n, op) in &ops {
        let _ = writeln!(
            body,
            "brrtrouter_sse_streams{{operation=\"{}\"}} {}",
            escape(n),
            op.open.load(Ordering::Relaxed)
        );
    }
    let _ = writeln!(body, "# HELP brrtrouter_sse_principals Distinct principals (verified subjects) with a relayed stream open now.");
    let _ = writeln!(body, "# TYPE brrtrouter_sse_principals gauge");
    for (n, op) in &ops {
        let _ = writeln!(
            body,
            "brrtrouter_sse_principals{{operation=\"{}\"}} {}",
            escape(n),
            op.principals()
        );
    }
    let _ = writeln!(
        body,
        "# HELP brrtrouter_sse_opened_total Relayed event streams opened."
    );
    let _ = writeln!(body, "# TYPE brrtrouter_sse_opened_total counter");
    for (n, op) in &ops {
        let _ = writeln!(
            body,
            "brrtrouter_sse_opened_total{{operation=\"{}\"}} {}",
            escape(n),
            op.opened.load(Ordering::Relaxed)
        );
    }
    let _ = writeln!(
        body,
        "# HELP brrtrouter_sse_closed_total Relayed event streams closed, by reason."
    );
    let _ = writeln!(body, "# TYPE brrtrouter_sse_closed_total counter");
    for (n, op) in &ops {
        for r in CloseReason::ALL {
            let _ = writeln!(
                body,
                "brrtrouter_sse_closed_total{{operation=\"{}\",reason=\"{}\"}} {}",
                escape(n),
                r.as_str(),
                op.closed[r as usize].load(Ordering::Relaxed)
            );
        }
    }
    let _ = writeln!(
        body,
        "# HELP brrtrouter_sse_duration_seconds How long relayed event streams lived."
    );
    let _ = writeln!(body, "# TYPE brrtrouter_sse_duration_seconds histogram");
    for (n, op) in &ops {
        let n = escape(n);
        for (i, b) in BUCKETS.iter().enumerate() {
            let _ = writeln!(
                body,
                "brrtrouter_sse_duration_seconds_bucket{{operation=\"{n}\",le=\"{b}\"}} {}",
                op.buckets[i].load(Ordering::Relaxed)
            );
        }
        let count = op.count.load(Ordering::Relaxed);
        let _ = writeln!(
            body,
            "brrtrouter_sse_duration_seconds_bucket{{operation=\"{n}\",le=\"+Inf\"}} {count}"
        );
        #[allow(clippy::cast_precision_loss)]
        let sum = op.sum_micros.load(Ordering::Relaxed) as f64 / 1e6;
        let _ = writeln!(
            body,
            "brrtrouter_sse_duration_seconds_sum{{operation=\"{n}\"}} {sum}"
        );
        let _ = writeln!(
            body,
            "brrtrouter_sse_duration_seconds_count{{operation=\"{n}\"}} {count}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(name: &str) -> (i64, usize, u64) {
        let op = operation(name);
        (
            op.open.load(Ordering::Relaxed),
            op.principals(),
            op.opened.load(Ordering::Relaxed),
        )
    }

    #[test]
    fn principals_and_streams_return_to_zero_after_churn() {
        let name = "__test_churn";
        let claims = |sub: &str| serde_json::json!({ "sub": sub });
        let (a, b) = (
            principal_key(Some(&claims("alice"))),
            principal_key(Some(&claims("bob"))),
        );
        assert!(a.is_some() && a != b);
        assert_eq!(
            principal_key(None),
            None,
            "no verified claims, no principal"
        );
        assert_eq!(principal_key(Some(&serde_json::json!({ "sub": "" }))), None);
        for round in 0..50 {
            let mut open: Vec<StreamGuard> = [a, a, b, None]
                .into_iter()
                .map(|p| opened(name, p))
                .collect();
            assert_eq!(snapshot(name).0, 4);
            assert_eq!(snapshot(name).1, 2, "two tabs of alice count once");
            // a client dropped mid-stream, one that fell behind, one the downstream ended, one relay that never closed
            open.pop().map(|g| g.close(CloseReason::ClientClosed));
            open.pop().map(|g| g.close(CloseReason::SlowClient));
            open.pop().map(|g| g.close(CloseReason::DownstreamEnded));
            drop(open); // recorded as an error
            assert_eq!(snapshot(name), (0, 0, 4 * (round + 1)), "round {round}");
        }
        let mut text = String::new();
        render_prometheus(&mut text);
        assert!(text.contains("brrtrouter_sse_streams{operation=\"__test_churn\"} 0"));
        assert!(text.contains("brrtrouter_sse_principals{operation=\"__test_churn\"} 0"));
        assert!(text.contains(
            "brrtrouter_sse_closed_total{operation=\"__test_churn\",reason=\"error\"} 50"
        ));
        assert!(
            text.contains("brrtrouter_sse_duration_seconds_count{operation=\"__test_churn\"} 200")
        );
    }

    #[test]
    fn labels_are_bounded() {
        let mut ops = Vec::new();
        assert!(std::ptr::eq(
            operation_in(&mut ops, ""),
            operation_in(&mut ops, "_unmatched")
        ));
        for i in 0..MAX_OPERATIONS + 5 {
            let _ = operation_in(&mut ops, &format!("op_{i}"));
        }
        assert_eq!(ops.len(), MAX_OPERATIONS + 1);
        assert!(ops.iter().any(|(n, _)| n == "_other"));
        assert!(std::ptr::eq(
            operation_in(&mut ops, "op_999"),
            operation_in(&mut ops, "op_1000")
        ));
    }
}
