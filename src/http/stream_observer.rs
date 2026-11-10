//! A hook that sees each relayed event stream from open to close ([`super::proxy_sse`]).
//!
//! An application registers one [`StreamObserver`] with [`set_stream_observer`]. For every stream the proxy relays,
//! the observer's [`StreamObserver::opened`] is called once the downstream has answered with an event stream. It
//! receives a [`StreamOpen`]: the route's handler name, the claims the router verified for the request, and
//! read-only request headers and query parameters. It may return a [`StreamLease`]. The proxy then:
//!
//! - calls [`StreamLease::alive`] every `BRRTR_SSE_OBSERVER_ALIVE_SECS` seconds (default 30) while the stream is open,
//!   from the relay's supervising coroutine;
//! - calls [`StreamLease::closed`] exactly once when the stream ends, with the same [`CloseReason`] as
//!   `brrtrouter_sse_closed_total`: the client went away (also while the downstream was silent), the downstream
//!   ended, the idle timeout passed, the client fell behind, or anything else (`error`), including a relay that
//!   panicked or never started.
//!
//! What an observer does with that is the application's business (count live clients per user in a shared store,
//! say); this module knows nothing about users, tabs or browsers.
//!
//! **Claims.** [`StreamOpen::claims`] is `HandlerRequest::jwt_claims`, which the router fills only after the route's
//! security check passed. Nothing here decodes a token.
//!
//! **Cost.** With no observer registered, a stream pays one atomic load at open and nothing after. With one, the
//! calls above and nothing per frame.
//!
//! **Failures.** A panic in `opened` or `alive` is caught: the stream goes on (without a lease, if `opened`
//! panicked). `closed` is called from a destructor when the relay unwinds, so it must not panic.
//!
//! **Scope.** Streams relayed by the proxy. An in-process event stream handler sees its own end (its sender reports
//! the client gone) and needs no hook.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::dispatcher::HeaderVec;
use crate::router::ParamVec;

pub use super::sse_metrics::CloseReason;

/// Seconds between [`StreamLease::alive`] calls (default 30, at least 1).
pub const ALIVE_ENV: &str = "BRRTR_SSE_OBSERVER_ALIVE_SECS";
const DEFAULT_ALIVE: Duration = Duration::from_secs(30);

/// What an observer learns about a stream when it opens.
pub struct StreamOpen<'a> {
    handler: &'a str,
    claims: Option<&'a Value>,
    headers: &'a HeaderVec,
    query: &'a ParamVec,
}

impl<'a> StreamOpen<'a> {
    /// The route's handler name as the router resolved it (its `operationId`, or an `x-handler` extension).
    #[must_use]
    pub fn handler(&self) -> &'a str {
        self.handler
    }

    /// The claims the router verified for this request, if the route has security and the check passed.
    #[must_use]
    pub fn claims(&self) -> Option<&'a Value> {
        self.claims
    }

    /// The verified subject (`sub` claim), if there is one.
    #[must_use]
    pub fn subject(&self) -> Option<&'a str> {
        self.claims?.get("sub")?.as_str()
    }

    /// A request header (case-insensitive name), as the client sent it after any request hook ran.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&'a str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// A query parameter of the stream request (the first, if repeated).
    #[must_use]
    pub fn query_param(&self, name: &str) -> Option<&'a str> {
        self.query
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Sees relayed event streams open; registered once per process with [`set_stream_observer`].
pub trait StreamObserver: Send + Sync {
    /// A stream opened. Return a lease to hear about it until it ends, or `None` to ignore it.
    fn opened(&self, open: &StreamOpen<'_>) -> Option<Box<dyn StreamLease>>;
}

/// One open stream, as an observer holds it.
pub trait StreamLease: Send {
    /// Called every `BRRTR_SSE_OBSERVER_ALIVE_SECS` while the stream is open.
    fn alive(&mut self) {}

    /// Called exactly once when the stream ends. Must not panic: it may run while the relay unwinds.
    fn closed(self: Box<Self>, reason: CloseReason);
}

static OBSERVER: OnceLock<Arc<dyn StreamObserver>> = OnceLock::new();

/// Register the process's stream observer. Only the first registration takes effect.
///
/// # Errors
///
/// Returns the observer back when one is already registered.
pub fn set_stream_observer(
    observer: Arc<dyn StreamObserver>,
) -> Result<(), Arc<dyn StreamObserver>> {
    OBSERVER.set(observer)
}

fn alive_every() -> Duration {
    static EVERY: OnceLock<Duration> = OnceLock::new();
    *EVERY.get_or_init(|| {
        std::env::var(ALIVE_ENV)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map_or(DEFAULT_ALIVE, |secs| Duration::from_secs(secs.max(1)))
    })
}

/// The proxy's handle on an observer's lease: ticks it, and closes it exactly once (with `error` if dropped
/// unclosed, which is how a relay that panicked or never started reports).
pub(crate) struct Lease {
    inner: Option<Box<dyn StreamLease>>,
    every: Duration,
    last: Instant,
}

/// Ask the registered observer about a stream that just opened. `None` (one atomic load) when there is no
/// observer, when it ignores the stream, or when it panicked.
pub(crate) fn open(
    handler: &str,
    claims: Option<&Value>,
    headers: &HeaderVec,
    query: &ParamVec,
) -> Option<Lease> {
    open_with(
        OBSERVER.get()?.as_ref(),
        handler,
        claims,
        headers,
        query,
        alive_every(),
    )
}

pub(crate) fn open_with(
    observer: &dyn StreamObserver,
    handler: &str,
    claims: Option<&Value>,
    headers: &HeaderVec,
    query: &ParamVec,
    every: Duration,
) -> Option<Lease> {
    let open = StreamOpen {
        handler,
        claims,
        headers,
        query,
    };
    let inner = catch_unwind(AssertUnwindSafe(|| observer.opened(&open)))
        .ok()
        .flatten()?;
    Some(Lease {
        inner: Some(inner),
        every,
        last: Instant::now(),
    })
}

impl Lease {
    /// Call `alive` if its interval has passed since the last call (or the open).
    pub(crate) fn tick(&mut self, now: Instant) {
        if now.duration_since(self.last) < self.every {
            return;
        }
        self.last = now;
        if let Some(inner) = self.inner.as_mut() {
            // A panicking observer must not end the stream.
            let _ = catch_unwind(AssertUnwindSafe(|| inner.alive()));
        }
    }

    /// The stream ended for `reason`.
    pub(crate) fn close(mut self, reason: CloseReason) {
        if let Some(inner) = self.inner.take() {
            let _ = catch_unwind(AssertUnwindSafe(move || inner.closed(reason)));
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.closed(CloseReason::Error);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Coverage: the lease's own contract, without a proxy (the relay's reasons, end to end, are in `proxy.rs` and
    //! `tests/sse_stream_observer_tests.rs`).
    //!
    //! - What `open` hands the observer: handler, verified subject, headers.
    //! - An observer that ignores a stream, or panics in `opened`, leaves no lease; one that panics in `alive` does
    //!   not end the stream.
    //! - `alive` runs on its interval only; `closed` runs exactly once, with the reason given, or `error` when the
    //!   lease is dropped unclosed.
    //!
    //! Observers here are local values passed to `open_with`, never the process-wide registration, so tests do not
    //! see each other's streams.
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[derive(Default)]
    struct Record {
        seen: Mutex<Vec<(String, Option<String>, Option<String>)>>,
        alive: AtomicUsize,
        closed: Mutex<Vec<CloseReason>>,
    }

    struct Recorder(Arc<Record>);
    struct RecLease(Arc<Record>);

    impl StreamObserver for Recorder {
        fn opened(&self, open: &StreamOpen<'_>) -> Option<Box<dyn StreamLease>> {
            if open.handler() == "ignored" {
                return None;
            }
            if open.handler() == "panics" {
                panic!("observer failed");
            }
            self.0.seen.lock().unwrap().push((
                open.handler().to_string(),
                open.subject().map(str::to_string),
                open.header("x-tab").map(str::to_string),
            ));
            Some(Box::new(RecLease(Arc::clone(&self.0))))
        }
    }

    impl StreamLease for RecLease {
        fn alive(&mut self) {
            if self.0.alive.fetch_add(1, Ordering::SeqCst) == 0 {
                panic!("first alive fails");
            }
        }
        fn closed(self: Box<Self>, reason: CloseReason) {
            self.0.closed.lock().unwrap().push(reason);
        }
    }

    fn headers() -> HeaderVec {
        let mut h = HeaderVec::new();
        h.push((Arc::from("X-Tab"), "t1".to_string()));
        h
    }

    #[test]
    fn the_observer_sees_handler_subject_and_headers() {
        let rec = Arc::new(Record::default());
        let claims = serde_json::json!({ "sub": "user-1" });
        let lease = open_with(
            &Recorder(Arc::clone(&rec)),
            "get_stream",
            Some(&claims),
            &headers(),
            &ParamVec::new(),
            DEFAULT_ALIVE,
        );
        assert!(lease.is_some());
        assert_eq!(
            rec.seen.lock().unwrap()[0],
            (
                "get_stream".into(),
                Some("user-1".into()),
                Some("t1".into())
            )
        );
        lease.unwrap().close(CloseReason::DownstreamEnded);
        assert_eq!(
            *rec.closed.lock().unwrap(),
            vec![CloseReason::DownstreamEnded]
        );
    }

    #[test]
    fn no_claims_means_no_subject() {
        let rec = Arc::new(Record::default());
        let lease = open_with(
            &Recorder(Arc::clone(&rec)),
            "s",
            None,
            &HeaderVec::new(),
            &ParamVec::new(),
            DEFAULT_ALIVE,
        );
        assert_eq!(rec.seen.lock().unwrap()[0].1, None);
        drop(lease);
    }

    #[test]
    fn ignored_or_panicking_opens_leave_no_lease() {
        let rec = Arc::new(Record::default());
        let obs = Recorder(Arc::clone(&rec));
        assert!(open_with(
            &obs,
            "ignored",
            None,
            &HeaderVec::new(),
            &ParamVec::new(),
            DEFAULT_ALIVE
        )
        .is_none());
        assert!(open_with(
            &obs,
            "panics",
            None,
            &HeaderVec::new(),
            &ParamVec::new(),
            DEFAULT_ALIVE
        )
        .is_none());
        assert!(rec.closed.lock().unwrap().is_empty());
    }

    #[test]
    fn alive_runs_on_its_interval_and_survives_a_panic() {
        let rec = Arc::new(Record::default());
        let every = Duration::from_secs(30);
        let mut lease = open_with(
            &Recorder(Arc::clone(&rec)),
            "s",
            None,
            &HeaderVec::new(),
            &ParamVec::new(),
            every,
        )
        .unwrap();
        let t0 = lease.last;
        lease.tick(t0 + Duration::from_secs(10));
        assert_eq!(rec.alive.load(Ordering::SeqCst), 0);
        lease.tick(t0 + Duration::from_secs(30)); // panics inside, caught
        lease.tick(t0 + Duration::from_secs(45));
        assert_eq!(rec.alive.load(Ordering::SeqCst), 1);
        lease.tick(t0 + Duration::from_secs(60));
        assert_eq!(rec.alive.load(Ordering::SeqCst), 2);
        lease.close(CloseReason::ClientClosed);
        assert_eq!(*rec.closed.lock().unwrap(), vec![CloseReason::ClientClosed]);
    }

    #[test]
    fn a_lease_dropped_unclosed_reports_error_once() {
        let rec = Arc::new(Record::default());
        let lease = open_with(
            &Recorder(Arc::clone(&rec)),
            "s",
            None,
            &HeaderVec::new(),
            &ParamVec::new(),
            DEFAULT_ALIVE,
        );
        drop(lease);
        assert_eq!(*rec.closed.lock().unwrap(), vec![CloseReason::Error]);
    }
}
