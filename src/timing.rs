//! Operation timing: BRRTRouter's request path and outbound calls in the shared
//! `op_duration_seconds{component, op, outcome}` histogram (`microscaler-timing`), rendered on
//! `/metrics` with every other timed operation in the process. A service's own
//! `#[microscaler_timing::timed]` functions appear beside these with no further wiring.
//!
//! | component | op | what |
//! |---|---|---|
//! | `brrtrouter.queue` | operationId | dispatcher queued the request until its handler took it |
//! | `brrtrouter.handler` | operationId | the handler ran (outcome `err` if it panicked) |
//! | `brrtrouter.client` / `brrtrouter.proxy` | `pool_checkout` | the pool handed out a connection (reused, new, or after waiting) |
//! | | `pool_wait` | waited for a connection because the per-host or total limit was reached |
//! | | `connect` / `connect_tls` | opened a new connection (TCP, TLS handshake) |
//! | | `request_write` | wrote the request head and body |
//! | | `response_headers` | request written until the response head was read: the server plus the network |
//! | | `response_body` | read the response body |
//! | | `call` | the whole call, retries included |
//! | `brrtrouter.net` | `resolve_cached` / `resolve` | [`crate::net`] answered from its cache / had to look the name up |
//! | `brrtrouter.proxy` | `dns_cached` / `dns_resolve` | the proxy's downstream address cache |
//! | `brrtrouter.auth` | `jwt_verify` | validating a bearer JWT against the JWKS (`err`: refused) |
//!
//! `brrtrouter.client` is the `fetch_*` pool, `brrtrouter.proxy` the BFF proxy pool. Labels are
//! static: operationIds from the spec (interned once, when the handler starts), never paths,
//! hosts or ids. Recording is a few atomic adds: nothing here blocks a `may` worker.

use std::sync::Arc;
use std::time::Duration;

use may_minihttp::client::{ClientErrorKind, ClientEvent, ClientObserver};
use microscaler_timing::{intern, Outcome, Site, Timer};

use crate::dispatcher::HandlerRequest;

/// Queue-wait and execution sites of one handler, interned when its coroutine (or worker) starts.
pub struct HandlerTiming {
    queue: &'static Site,
    handler: &'static Site,
}

impl HandlerTiming {
    /// The sites for `handler_name` (the spec's operationId). Takes a lock: call once per handler
    /// coroutine or worker, not per request.
    #[must_use]
    pub fn new(handler_name: &str) -> Self {
        Self {
            queue: intern("brrtrouter.queue", handler_name),
            handler: intern("brrtrouter.handler", handler_name),
        }
    }

    /// The handler took `req` off its queue: record how long it waited there.
    pub fn dequeued(&self, req: &HandlerRequest) {
        if let Some(guard) = &req.queue_guard {
            self.queue.record(Outcome::Ok, guard.queued_for());
        }
    }

    /// Start timing the handler; finish with [`Self::finish`] (a panic records `err`).
    pub fn start(&self) -> Timer {
        self.handler.start()
    }

    /// Record the handler's run: `panicked` records `err`.
    pub fn finish(timer: Timer, panicked: bool) {
        timer.finish(if panicked { Outcome::Err } else { Outcome::Ok });
    }
}

/// A handler coroutine's (or worker's) [`HandlerTiming`], interned on its first request: the
/// operationId comes with the request, and a coroutine serves one handler.
#[derive(Default)]
pub struct HandlerTimingSlot(Option<(String, HandlerTiming)>);

impl HandlerTimingSlot {
    /// The sites for `req`'s handler; interns only when the handler name changes (in practice,
    /// once per coroutine).
    pub fn for_request(&mut self, req: &HandlerRequest) -> &HandlerTiming {
        let stale = self
            .0
            .as_ref()
            .is_none_or(|(name, _)| name.as_str() != req.handler_name.as_str());
        if stale {
            self.0 = None;
        }
        let (_, timing) = self.0.get_or_insert_with(|| {
            (
                req.handler_name.clone(),
                HandlerTiming::new(&req.handler_name),
            )
        });
        timing
    }
}

/// The phases of one outbound pool's calls.
pub struct PhaseSites {
    pool_checkout: Site,
    pool_wait: Site,
    connect: Site,
    connect_tls: Site,
    request_write: Site,
    response_headers: Site,
    response_body: Site,
    call: Site,
}

impl PhaseSites {
    const fn new(component: &'static str) -> Self {
        Self {
            pool_checkout: Site::new(component, "pool_checkout"),
            pool_wait: Site::new(component, "pool_wait"),
            connect: Site::new(component, "connect"),
            connect_tls: Site::new(component, "connect_tls"),
            request_write: Site::new(component, "request_write"),
            response_headers: Site::new(component, "response_headers"),
            response_body: Site::new(component, "response_body"),
            call: Site::new(component, "call"),
        }
    }

    /// The body-read site, for callers that read a streaming response themselves.
    #[must_use]
    pub const fn response_body(&'static self) -> &'static Site {
        &self.response_body
    }
}

/// The `fetch_*` pool's phases.
pub static FETCH: PhaseSites = PhaseSites::new("brrtrouter.client");
/// The BFF proxy pool's phases.
pub static PROXY: PhaseSites = PhaseSites::new("brrtrouter.proxy");

/// `crate::net` answered from its cache.
pub static NET_RESOLVE_CACHED: Site = Site::new("brrtrouter.net", "resolve_cached");
/// `crate::net` looked the name up (or waited for a lookup in flight).
pub static NET_RESOLVE: Site = Site::new("brrtrouter.net", "resolve");
/// The proxy's downstream address cache answered.
pub static PROXY_DNS_CACHED: Site = Site::new("brrtrouter.proxy", "dns_cached");
/// The proxy resolved a downstream name.
pub static PROXY_DNS_RESOLVE: Site = Site::new("brrtrouter.proxy", "dns_resolve");
/// A bearer JWT validated against the JWKS (`err` when it was refused).
pub static JWT_VERIFY: Site = Site::new("brrtrouter.auth", "jwt_verify");

fn outcome_of(error: Option<ClientErrorKind>) -> Outcome {
    match error {
        None => Outcome::Ok,
        Some(ClientErrorKind::Timeout) => Outcome::Timeout,
        Some(_) => Outcome::Err,
    }
}

fn outcome_of_io(result: &std::io::Result<impl Sized>) -> Outcome {
    match result {
        Ok(_) => Outcome::Ok,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            Outcome::Timeout
        }
        Err(_) => Outcome::Err,
    }
}

/// Record a [`crate::net`] lookup: `cached` when it was answered from the cache.
pub fn record_resolve(cached: bool, result: &std::io::Result<impl Sized>, took: Duration) {
    let site = if cached {
        &NET_RESOLVE_CACHED
    } else {
        &NET_RESOLVE
    };
    site.record(outcome_of_io(result), took);
}

/// Records the pooled client's lifecycle events into one pool's [`PhaseSites`].
pub struct PhaseObserver(&'static PhaseSites);

impl PhaseObserver {
    /// An observer for the pool whose phases are `sites`, ready for `Client::builder().observer`.
    #[must_use]
    pub fn for_pool(sites: &'static PhaseSites) -> Arc<dyn ClientObserver> {
        Arc::new(Self(sites))
    }
}

impl ClientObserver for PhaseObserver {
    fn observe(&self, event: ClientEvent<'_>) {
        let s = self.0;
        match event {
            ClientEvent::CheckoutCompleted {
                duration, error, ..
            } => s.pool_checkout.record(outcome_of(error), duration),
            ClientEvent::PoolWaited {
                duration,
                timed_out,
                ..
            } => s.pool_wait.record(
                if timed_out {
                    Outcome::Timeout
                } else {
                    Outcome::Ok
                },
                duration,
            ),
            ClientEvent::ConnectionCompleted {
                duration,
                tls,
                error,
                ..
            } => if tls { &s.connect_tls } else { &s.connect }.record(outcome_of(error), duration),
            ClientEvent::RequestWritten {
                duration, error, ..
            } => s.request_write.record(outcome_of(error), duration),
            ClientEvent::ResponseHeadersReceived {
                duration, error, ..
            } => s.response_headers.record(outcome_of(error), duration),
            ClientEvent::ResponseBodyRead {
                duration, error, ..
            } => s.response_body.record(outcome_of(error), duration),
            ClientEvent::RequestCompleted { total_duration, .. } => {
                s.call.record(Outcome::Ok, total_duration);
            }
            ClientEvent::RequestFailed {
                error,
                total_duration,
                ..
            } => s.call.record(outcome_of(Some(error)), total_duration),
            ClientEvent::RequestCancelled { total_duration, .. } => {
                s.call.record(Outcome::Err, total_duration);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(component: &str, op: &str, outcome: Outcome) -> u64 {
        microscaler_timing::snapshot()
            .into_iter()
            .filter(|s| s.component == component && s.op == op && s.outcome == outcome)
            .map(|s| s.count())
            .sum()
    }

    #[test]
    fn handler_sites_are_interned_once_per_name() {
        let a = HandlerTiming::new("timingTestOp");
        let b = HandlerTiming::new("timingTestOp");
        assert!(std::ptr::eq(a.handler, b.handler));
        assert!(std::ptr::eq(a.queue, b.queue));
        let t = a.start();
        HandlerTiming::finish(t, true);
        assert_eq!(count("brrtrouter.handler", "timingTestOp", Outcome::Err), 1);
    }

    #[test]
    fn resolve_split_by_cache() {
        let ok: std::io::Result<()> = Ok(());
        let timed_out: std::io::Result<()> =
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "slow"));
        let before = count("brrtrouter.net", "resolve_cached", Outcome::Ok);
        record_resolve(true, &ok, Duration::from_micros(5));
        record_resolve(false, &timed_out, Duration::from_millis(30));
        assert_eq!(
            count("brrtrouter.net", "resolve_cached", Outcome::Ok),
            before + 1
        );
        assert!(count("brrtrouter.net", "resolve", Outcome::Timeout) >= 1);
    }
}
