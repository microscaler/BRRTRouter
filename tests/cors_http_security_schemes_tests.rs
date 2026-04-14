#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

//! HTTP CORS + **global** OpenAPI security beyond single-scheme `ApiKeyHeader` (Bearer, cookie API
//! key, OR ApiKey/Bearer). Complements `cors_http_conformance_tests.rs` (pet_store
//! `examples/openapi.yaml`). See `docs/CORS_IMPLEMENTATION_AUDIT.md` §3 and §4.

use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec},
    middleware::{CorsMiddlewareBuilder, MetricsMiddleware, TracingMiddleware},
    router::Router,
    server::AppService,
    BearerJwtProvider, SecurityProvider, SecurityRequest,
};
use http::Method;
use serde_json::json;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

mod tracing_util;
use tracing_util::TestTracing;
mod common;
use common::http::send_request;

fn parse_response_status(resp: &str) -> u16 {
    for line in resp.split("\r\n\r\n").next().unwrap_or("").lines() {
        if line.starts_with("HTTP/1.1") || line.starts_with("HTTP/1.0") {
            return line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Same shape as `BearerJwtProvider` tests: third segment must match configured signature.
fn make_dummy_bearer_token(scope: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};
    let header = general_purpose::STANDARD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = general_purpose::STANDARD.encode(format!(r#"{{"scope":"{scope}"}}"#));
    format!("{header}.{payload}.sig")
}

struct StaticCookieApiKeyProvider {
    key: String,
}

impl SecurityProvider for StaticCookieApiKeyProvider {
    fn validate(
        &self,
        scheme: &SecurityScheme,
        _scopes: &[String],
        req: &SecurityRequest,
    ) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } if location.as_str() == "cookie" => req
                .get_cookie(name)
                .map(|v| v == self.key)
                .unwrap_or(false),
            _ => false,
        }
    }
}

struct MinimalCorsFixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl MinimalCorsFixture {
    fn new(spec_rel: &str, register_auth: impl FnOnce(&mut AppService)) -> Self {
        std::env::set_var("BRRTR_STACK_SIZE", "0x8000");
        let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
        may::config().set_stack_size(config.stack_size);
        let tracing = TestTracing::init();
        let spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(spec_rel);
        let (routes, schemes, _slug) =
            brrtrouter::load_spec_full(spec_path.to_str().unwrap()).unwrap();
        let router = Arc::new(RwLock::new(Router::new(routes.clone())));
        let mut dispatcher = Dispatcher::new();
        let metrics = Arc::new(MetricsMiddleware::new());
        dispatcher.add_middleware(metrics.clone());
        let cors = Arc::new(
            CorsMiddlewareBuilder::new()
                .allowed_origins(&["https://client.example"])
                .allowed_methods(&[Method::GET, Method::HEAD, Method::OPTIONS])
                .build()
                .unwrap()
                .with_metrics_sink(metrics.clone()),
        );
        dispatcher.add_middleware(cors);
        dispatcher.add_middleware(Arc::new(TracingMiddleware));
        unsafe {
            dispatcher.register_handler("echo_get", |req: HandlerRequest| {
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({"ok": true}),
                });
            });
            dispatcher.register_handler("echo_options", |req: HandlerRequest| {
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({"ok": true}),
                });
            });
        }
        let mut service = AppService::new(
            router,
            Arc::new(RwLock::new(dispatcher)),
            schemes,
            spec_path,
            None,
            None,
        );
        service.set_metrics_middleware(metrics);
        register_auth(&mut service);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(service).start(addr).unwrap();
        handle.wait_ready().unwrap();
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for MinimalCorsFixture {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.stop();
        }
    }
}

struct StaticHeaderApiKeyProvider {
    key: String,
}

impl SecurityProvider for StaticHeaderApiKeyProvider {
    fn validate(
        &self,
        scheme: &SecurityScheme,
        _scopes: &[String],
        req: &SecurityRequest,
    ) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } if location.as_str() == "header" => req
                .get_header(&name.to_ascii_lowercase())
                .map(|v| v == self.key)
                .unwrap_or(false),
            _ => false,
        }
    }
}

#[test]
fn http_cors_preflight_global_bearer_returns_401_without_authorization() {
    let f = MinimalCorsFixture::new(
        "tests/fixtures/cors_global_bearer.yaml",
        |service| {
            service.register_security_provider(
                "BearerAuth",
                Arc::new(BearerJwtProvider::new("sig")),
            );
        },
    );
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /echo HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    assert_eq!(
        parse_response_status(&resp),
        401,
        "global Bearer: preflight without Authorization must fail auth: {resp}"
    );
}

#[test]
fn http_cors_preflight_global_bearer_returns_200_with_valid_bearer() {
    let token = make_dummy_bearer_token("");
    let f = MinimalCorsFixture::new(
        "tests/fixtures/cors_global_bearer.yaml",
        |service| {
            service.register_security_provider(
                "BearerAuth",
                Arc::new(BearerJwtProvider::new("sig")),
            );
        },
    );
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /echo HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\
         Authorization: Bearer {token}\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    assert_eq!(parse_response_status(&resp), 200, "preflight with Bearer: {resp}");
    let lower = resp.to_ascii_lowercase();
    assert!(
        lower.contains("access-control-allow-origin: https://client.example"),
        "expected ACAO: {resp}"
    );
}

#[test]
fn http_cors_preflight_global_cookie_api_key_returns_401_without_cookie() {
    let f = MinimalCorsFixture::new(
        "tests/fixtures/cors_global_apikey_cookie.yaml",
        |service| {
            service.register_security_provider(
                "SessionCookie",
                Arc::new(StaticCookieApiKeyProvider {
                    key: "test123".into(),
                }),
            );
        },
    );
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /echo HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    assert_eq!(
        parse_response_status(&resp),
        401,
        "global cookie API key: preflight without cookie must fail: {resp}"
    );
}

#[test]
fn http_cors_preflight_global_cookie_api_key_returns_200_with_session_cookie() {
    let f = MinimalCorsFixture::new(
        "tests/fixtures/cors_global_apikey_cookie.yaml",
        |service| {
            service.register_security_provider(
                "SessionCookie",
                Arc::new(StaticCookieApiKeyProvider {
                    key: "test123".into(),
                }),
            );
        },
    );
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /echo HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\
         Cookie: session=test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    assert_eq!(
        parse_response_status(&resp),
        200,
        "preflight with session cookie: {resp}"
    );
    let lower = resp.to_ascii_lowercase();
    assert!(
        lower.contains("access-control-allow-origin: https://client.example"),
        "expected ACAO: {resp}"
    );
}

/// OpenAPI global `security` lists two requirements → **either** API key **or** Bearer satisfies.
#[test]
fn http_cors_preflight_security_or_succeeds_with_api_key_only() {
    let f = MinimalCorsFixture::new(
        "tests/fixtures/cors_security_or_apikey_bearer.yaml",
        |service| {
            service.register_security_provider(
                "ApiKeyHeader",
                Arc::new(StaticHeaderApiKeyProvider {
                    key: "test123".into(),
                }),
            );
            service.register_security_provider(
                "BearerAuth",
                Arc::new(BearerJwtProvider::new("sig")),
            );
        },
    );
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /echo HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    assert_eq!(parse_response_status(&resp), 200, "OR security with API key: {resp}");
}

#[test]
fn http_cors_preflight_security_or_succeeds_with_bearer_only() {
    let token = make_dummy_bearer_token("");
    let f = MinimalCorsFixture::new(
        "tests/fixtures/cors_security_or_apikey_bearer.yaml",
        |service| {
            service.register_security_provider(
                "ApiKeyHeader",
                Arc::new(StaticHeaderApiKeyProvider {
                    key: "test123".into(),
                }),
            );
            service.register_security_provider(
                "BearerAuth",
                Arc::new(BearerJwtProvider::new("sig")),
            );
        },
    );
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /echo HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\
         Authorization: Bearer {token}\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    assert_eq!(parse_response_status(&resp), 200, "OR security with Bearer: {resp}");
}
