//! Outbound calls are traced end to end between two BRRTRouter services: the edge's trace context
//! reaches the BFF, the BFF's proxied call and fetch are client spans under its request span, the
//! downstream's request span is a child of the client span, and may_minihttp's connect/write/wait/
//! body spans hang under the client span. One test binary per global subscriber: the servers'
//! coroutines run on worker threads, so a thread-scoped subscriber would not see them.

#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse};
use brrtrouter::router::Router;
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
use opentelemetry::trace::{SpanId, TraceId, TracerProvider as _};
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::trace::{SdkTracerProvider, SpanData, SpanProcessor};
use tracing_subscriber::prelude::*;

const EDGE_TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const EDGE_SPAN: &str = "00f067aa0ba902b7";

#[derive(Clone, Debug, Default)]
struct Collect(Arc<Mutex<Vec<SpanData>>>);

impl SpanProcessor for Collect {
    fn on_start(&self, _span: &mut opentelemetry_sdk::trace::Span, _cx: &opentelemetry::Context) {}
    fn on_end(&self, span: SpanData) {
        self.0.lock().unwrap().push(span);
    }
    fn force_flush(&self) -> Result<(), OTelSdkError> {
        Ok(())
    }
    fn shutdown(&self) -> Result<(), OTelSdkError> {
        Ok(())
    }
    fn shutdown_with_timeout(&self, _timeout: Duration) -> Result<(), OTelSdkError> {
        Ok(())
    }
}

fn spans() -> &'static Collect {
    static SPANS: OnceLock<Collect> = OnceLock::new();
    SPANS.get_or_init(|| {
        let collect = Collect::default();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(collect.clone())
            .build();
        let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));
        tracing_subscriber::registry()
            .with(layer)
            .try_init()
            .expect("global subscriber");
        std::mem::forget(provider);
        collect
    })
}

/// Traceparents the downstream received.
static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn serve(
    spec: &str,
    handler: impl Fn(HandlerRequest) + Send + Sync + Clone + 'static,
) -> (ServerHandle, SocketAddr) {
    may::config().set_stack_size(2 * 1024 * 1024 / std::mem::size_of::<usize>());
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join("openapi.json");
    std::fs::write(&path, spec).unwrap();
    let path = path.to_str().unwrap().to_string();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(&path).expect("spec");
    let mut dispatcher = Dispatcher::new();
    for r in &routes {
        let handler = handler.clone();
        unsafe {
            dispatcher.register_handler(&r.handler_name, move |req: HandlerRequest| handler(req));
        }
    }
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        schemes,
        path.into(),
        None,
        None,
    );
    service.resolve_security(&routes);
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let handle = HttpServer(service).start(addr).expect("server");
    for _ in 0..200 {
        if TcpStream::connect(addr).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (handle, addr)
}

const DOWN_SPEC: &str = r#"{"openapi":"3.1.0","info":{"title":"down","version":"1"},"paths":{
    "/items/{id}":{"get":{"operationId":"get_item","parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],
    "responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"object"}}}}}}}}}"#;

const BFF_SPEC: &str = r#"{"openapi":"3.1.0","info":{"title":"bff","version":"1"},"paths":{
    "/bff/items/{id}":{"get":{"operationId":"bff_item","parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],
    "responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"object"}}}}}}}}}"#;

fn get(addr: SocketAddr, path: &str, traceparent: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: bff\r\ntraceparent: {traceparent}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    // The status line is all this test needs; the server may keep the connection open.
    let mut out = Vec::new();
    let mut chunk = [0_u8; 4096];
    while !out.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(id: SpanId) -> String {
    format!("{id}")
}

#[test]
fn a_proxied_call_and_a_fetch_are_one_trace_across_two_services() {
    let collected = spans();
    let (_down, down_addr) = serve(DOWN_SPEC, |req: HandlerRequest| {
        if let Some((_, tp)) = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("traceparent"))
        {
            SEEN.lock().unwrap().push(tp.clone());
        }
        req.reply(HandlerResponse::json(200, serde_json::json!({"ok": true})));
    });
    std::env::set_var("SERVICE_HTTP_PORT", down_addr.port().to_string());
    std::env::remove_var("POD_NAMESPACE");
    let fetch_url = format!("http://127.0.0.1:{}/items/fetched", down_addr.port());
    let (_bff, bff_addr) = serve(BFF_SPEC, move |req: HandlerRequest| {
        let proxied = brrtrouter::http::proxy_untyped(&req, "127.0.0.1", "/items/{id}");
        let fetched =
            brrtrouter::http::fetch_get(&fetch_url, &brrtrouter::http::HttpFetchOptions::default());
        assert_eq!(fetched.map(|(status, _)| status).ok(), Some(200));
        req.reply(proxied);
    });

    let answer = get(
        bff_addr,
        "/bff/items/42",
        &format!("00-{EDGE_TRACE}-{EDGE_SPAN}-01"),
    );
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");

    // Spans end when the servers finish their turns; give the exporters a moment.
    let trace = TraceId::from_hex(EDGE_TRACE).unwrap();
    let mut ours: Vec<SpanData> = Vec::new();
    for _ in 0..200 {
        ours = collected
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.span_context.trace_id() == trace)
            .cloned()
            .collect();
        if ours.iter().filter(|s| s.name == "GET /items/{id}").count() >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let names: Vec<String> = ours
        .iter()
        .map(|s| format!("{} parent={}", s.name, s.parent_span_id))
        .collect();

    let by_id = |id: SpanId| ours.iter().find(|s| s.span_context.span_id() == id);
    // The BFF's server span is the edge's child.
    let bff = ours
        .iter()
        .find(|s| s.parent_span_id == SpanId::from_hex(EDGE_SPAN).unwrap())
        .unwrap_or_else(|| panic!("no BFF span under the edge: {names:#?}"));
    // Two client spans under it: the proxied call (named after its route) and the fetch.
    let clients: Vec<&SpanData> = ours
        .iter()
        .filter(|s| {
            s.parent_span_id == bff.span_context.span_id()
                && s.span_kind == opentelemetry::trace::SpanKind::Client
        })
        .collect();
    assert_eq!(clients.len(), 2, "{names:#?}");
    let proxied = clients
        .iter()
        .find(|s| s.name == "GET /items/{id}")
        .unwrap_or_else(|| panic!("proxied client span: {names:#?}"));
    assert!(
        proxied
            .attributes
            .iter()
            .any(|kv| kv.key.as_str() == "http.route" && kv.value.as_str() == "/items/{id}"),
        "{:?}",
        proxied.attributes
    );
    assert!(
        !proxied
            .attributes
            .iter()
            .any(|kv| kv.value.as_str().contains("/items/42")),
        "no ids in attributes: {:?}",
        proxied.attributes
    );
    // Each downstream server span is a child of a client span, and what the downstream received
    // names that client span.
    let seen = SEEN.lock().unwrap().clone();
    assert_eq!(seen.len(), 2, "{seen:?}");
    for client in &clients {
        let id = client.span_context.span_id();
        assert!(
            ours.iter().any(|s| s.parent_span_id == id
                && s.span_kind != opentelemetry::trace::SpanKind::Client
                && s.name.starts_with("GET /items")
                && by_id(s.parent_span_id).is_some()),
            "no downstream server span under {}: {names:#?}",
            client.name
        );
        assert!(
            seen.iter()
                .any(|tp| tp == &format!("00-{EDGE_TRACE}-{}-01", hex(id))),
            "traceparent for {}: {seen:?}",
            client.name
        );
        // may_minihttp's phase spans are the client span's children.
        for phase in ["http_client_write", "http_client_wait", "http_client_body"] {
            assert!(
                ours.iter()
                    .any(|s| s.name == phase && s.parent_span_id == id),
                "{phase} under {}: {names:#?}",
                client.name
            );
        }
    }
}
