//! Epic 02.1 (may_tracing): the request span is the handler coroutine's context.
//!
//! Handlers are long-lived coroutines fed over a channel, so the request span travels on
//! `HandlerRequest::span` and the handler loop runs each request under it with
//! `may_tracing::with_span`. Inside a handler, `may_tracing::current()` is therefore the
//! `http_request` span and `may_tracing::child_span!` nests under it. Nothing is ever
//! entered on a thread (ADR-0001).
//!
//! Coroutines only see the *global* subscriber, so this binary installs one.

#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::router::ParamVec;
use http::Method;
use may::sync::mpsc;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::span::{Attributes, Id};
use tracing::Subscriber;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

/// Records explicit parents of new spans (never reads the thread's contextual span).
#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<HashMap<u64, Option<u64>>>>);

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Recorder {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _: Context<'_, S>) {
        self.0
            .lock()
            .unwrap()
            .insert(id.into_u64(), attrs.parent().map(Id::into_u64));
    }
}

fn install() -> Recorder {
    static G: OnceLock<Recorder> = OnceLock::new();
    G.get_or_init(|| {
        let rec = Recorder::default();
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(rec.clone()))
            .expect("installed once");
        rec
    })
    .clone()
}

fn request(
    handler_name: &str,
    span: tracing::Span,
    reply_tx: mpsc::Sender<HandlerResponse>,
) -> HandlerRequest {
    HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/probe".to_string(),
        handler_name: handler_name.to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        raw_query: None,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
        queue_guard: None,
        span,
    }
}

#[test]
fn handler_current_span_is_the_request_span() {
    let rec = install();
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("ctx_probe", |req: HandlerRequest| {
            let current = may_tracing::current();
            let name = current.metadata().map(|m| m.name()).unwrap_or("none");
            let child = may_tracing::child_span!(tracing::Level::INFO, "probe_child");
            let body = json!({
                "current": name,
                "child_id": child.id().map(|i| i.into_u64()),
                "in_coroutine": may_tracing::is_in_coroutine(),
            });
            let _ = req
                .reply_tx
                .send(HandlerResponse::new(200, HeaderVec::new(), body));
        });
    }

    // With a request span on the HandlerRequest: current() is it, child nests under it.
    let req_span = tracing::info_span!(parent: None, "http_request", path = "/probe");
    let (tx, rx) = mpsc::channel();
    dispatcher
        .handlers
        .get("ctx_probe")
        .unwrap()
        .send(request("ctx_probe", req_span.clone(), tx))
        .unwrap();
    let resp = rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body["current"], "http_request");
    assert_eq!(resp.body["in_coroutine"], true);
    let child_id = resp.body["child_id"].as_u64().expect("child span enabled");
    let parent = rec.0.lock().unwrap().get(&child_id).copied().flatten();
    assert_eq!(
        parent,
        req_span.id().map(|i| i.into_u64()),
        "child_span! nests under the request span"
    );

    // Without one (Span::none()): a root, and nothing panics.
    let (tx, rx) = mpsc::channel();
    dispatcher
        .handlers
        .get("ctx_probe")
        .unwrap()
        .send(request("ctx_probe", tracing::Span::none(), tx))
        .unwrap();
    let resp = rx.recv().unwrap();
    assert_eq!(resp.body["current"], "none");
    let child_id = resp.body["child_id"].as_u64().unwrap();
    assert_eq!(
        rec.0.lock().unwrap().get(&child_id).copied().flatten(),
        None
    );
}

#[test]
fn handler_context_does_not_leak_between_requests() {
    install();
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("leak_probe", |req: HandlerRequest| {
            // a handler that sets its own context must not affect the next request
            let _g = may_tracing::set_current(tracing::info_span!(parent: None, "handler_private"));
            let name = may_tracing::current()
                .metadata()
                .map(|m| m.name())
                .unwrap_or("none");
            let _ = req.reply_tx.send(HandlerResponse::new(
                200,
                HeaderVec::new(),
                json!({ "seen": name }),
            ));
        });
    }
    let tx_h = dispatcher.handlers.get("leak_probe").unwrap().clone();
    let (tx, rx) = mpsc::channel();
    tx_h.send(request(
        "leak_probe",
        tracing::info_span!(parent: None, "first"),
        tx,
    ))
    .unwrap();
    assert_eq!(rx.recv().unwrap().body["seen"], "handler_private");
    let (tx, rx) = mpsc::channel();
    tx_h.send(request("leak_probe", tracing::Span::none(), tx))
        .unwrap();
    assert_eq!(rx.recv().unwrap().body["seen"], "handler_private");
    // and the test thread's own context is untouched
    assert!(may_tracing::current().is_none());
}
