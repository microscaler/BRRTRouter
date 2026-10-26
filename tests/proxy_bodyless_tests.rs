//! A proxied request without a body (a `POST /reset`, a `DELETE`) reaches a BRRTRouter downstream
//! with `Content-Length: 0`, not an open chunked body, and every one of many calls succeeds.
//!
//! pp evidence (26 Oct 2026): `POST /billing/test-subscriptions/reset` through trader-bff answered
//! 502 `reason: request` on ~40 % of calls while billing logged each as a 200; the downstream had
//! received `transfer-encoding: chunked` for a request with no body.

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::router::{ParamVec, Router};
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Headers each downstream request arrived with, lowercased `name: value` lines.
static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn downstream() -> (ServerHandle, SocketAddr) {
    may::config().set_stack_size(2 * 1024 * 1024 / std::mem::size_of::<usize>());
    let spec = r#"{"openapi":"3.1.0","info":{"title":"down","version":"1"},"paths":{
        "/reset":{"post":{"operationId":"reset","responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"object"}}}}}}},
        "/item":{"delete":{"operationId":"delete_item","responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"object"}}}}}}}}}"#;
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join("openapi.json");
    std::fs::write(&path, spec).unwrap();
    let path = path.to_str().unwrap().to_string();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(&path).expect("spec");
    let mut dispatcher = Dispatcher::new();
    for r in &routes {
        unsafe {
            dispatcher.register_handler(&r.handler_name, move |req: HandlerRequest| {
                let te = req
                    .headers
                    .iter()
                    .filter(|(k, _)| {
                        k.eq_ignore_ascii_case("transfer-encoding")
                            || k.eq_ignore_ascii_case("content-length")
                    })
                    .map(|(k, v)| format!("{}: {v}", k.to_ascii_lowercase()))
                    .collect::<Vec<_>>()
                    .join(", ");
                SEEN.lock().unwrap().push(te);
                req.reply(brrtrouter::dispatcher::HandlerResponse::json(
                    200,
                    serde_json::json!({"canceled": []}),
                ));
            });
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

fn request(method: http::Method, path: &str) -> HandlerRequest {
    let (tx, _rx) = may::sync::mpsc::channel();
    let mut headers = HeaderVec::new();
    // What a browser's bodyless POST through Envoy looks like to the BFF.
    headers.push((Arc::from("accept"), "*/*".to_string()));
    headers.push((Arc::from("content-length"), "0".to_string()));
    HandlerRequest {
        request_id: RequestId::new(),
        method,
        path: path.to_string(),
        handler_name: "bff".to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        raw_query: None,
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
        span: tracing::Span::none(),
    }
}

#[test]
fn bodyless_requests_are_sent_with_content_length_zero_and_all_succeed() {
    let (app, addr) = downstream();
    std::env::set_var("SERVICE_HTTP_PORT", addr.port().to_string());
    std::env::remove_var("POD_NAMESPACE");
    let mut failures = Vec::new();
    for (method, path) in [
        (http::Method::POST, "/reset"),
        (http::Method::DELETE, "/item"),
    ] {
        for _ in 0..200 {
            let res =
                brrtrouter::http::proxy_untyped(&request(method.clone(), path), "127.0.0.1", path);
            if res.status != 200 {
                failures.push(format!("{method} {path}: {} {:?}", res.status, res.body));
            }
        }
    }
    let seen = SEEN.lock().unwrap().clone();
    drop(app);
    assert!(
        failures.is_empty(),
        "{} failed, first: {:?}",
        failures.len(),
        failures.first()
    );
    assert!(
        seen.iter().all(|h| h == "content-length: 0"),
        "downstream saw: {:?}",
        seen.iter().find(|h| *h != "content-length: 0")
    );
}
