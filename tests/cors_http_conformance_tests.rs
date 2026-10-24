#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

//! HTTP-level CORS conformance: forwarded `Host`, Private Network Access, IDN (punycode) bytes,
//! and release-gate scenarios from `docs/CORS_IMPLEMENTATION_AUDIT.md` §3 (OpenAPI global
//! `ApiKeyHeader` security runs **before** dispatch; preflight must include the key when required).
//! Global **Bearer** and **cookie** API key: `tests/cors_http_security_schemes_tests.rs`.

use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{
    dispatcher::Dispatcher,
    middleware::{CorsMiddleware, CorsMiddlewareBuilder, MetricsMiddleware, TracingMiddleware},
    router::Router,
    server::AppService,
    SecurityProvider, SecurityRequest,
};
use http::Method;
use pet_store::registry;

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

mod tracing_util;
use tracing_util::TestTracing;
mod common;
use common::http::send_request;

fn parse_response_parts(resp: &str) -> (u16, String) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let mut status = 0u16;
    for line in headers.lines() {
        if line.starts_with("HTTP/1.1") || line.starts_with("HTTP/1.0") {
            status = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            break;
        }
    }
    (status, resp.to_string())
}

struct CorsHttpFixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl CorsHttpFixture {
    fn new(cors: Arc<CorsMiddleware>) -> Self {
        Self::with_after_cors(cors, Vec::new())
    }

    /// Like `new`, with extra middleware registered after CORS (the production
    /// position of rate limiting).
    fn with_after_cors(
        cors: Arc<CorsMiddleware>,
        after_cors: Vec<Arc<dyn brrtrouter::middleware::Middleware>>,
    ) -> Self {
        std::env::set_var("BRRTR_STACK_SIZE", "0x40000");
        let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
        may::config().set_stack_size(config.stack_size);
        let tracing = TestTracing::init();
        let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
        let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
        let mut dispatcher = Dispatcher::new();
        let metrics = Arc::new(MetricsMiddleware::new());
        dispatcher.add_middleware(metrics.clone());
        dispatcher.add_middleware(cors);
        for mw in after_cors {
            dispatcher.add_middleware(mw);
        }
        dispatcher.add_middleware(Arc::new(TracingMiddleware));
        unsafe {
            registry::register_from_spec(&mut dispatcher, &routes);
        }
        let mut service = AppService::new(
            router,
            Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
            schemes,
            PathBuf::from("examples/openapi.yaml"),
            Some(PathBuf::from("examples/pet_store/static_site")),
            Some(PathBuf::from("examples/pet_store/doc")),
        );
        service.set_metrics_middleware(metrics);

        struct ApiKeyProvider {
            key: String,
        }
        impl SecurityProvider for ApiKeyProvider {
            fn validate(
                &self,
                scheme: &SecurityScheme,
                _scopes: &[String],
                req: &SecurityRequest,
            ) -> bool {
                match scheme {
                    SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                        "header" => req
                            .get_header(&name.to_ascii_lowercase())
                            .map(|v| v == self.key)
                            .unwrap_or(false),
                        "query" => req.get_query(name).map(|v| v == self.key).unwrap_or(false),
                        "cookie" => req.get_cookie(name).map(|v| v == self.key).unwrap_or(false),
                        _ => false,
                    },
                    _ => false,
                }
            }
        }
        for (name, scheme) in service.security_schemes.clone() {
            if matches!(scheme, SecurityScheme::ApiKey { .. }) {
                service.register_security_provider(
                    &name,
                    Arc::new(ApiKeyProvider {
                        key: "test123".into(),
                    }),
                );
            }
        }

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

impl Drop for CorsHttpFixture {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.stop();
        }
    }
}

#[test]
fn http_cors_trusted_forwarded_host_treats_as_same_origin() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://api.example.com"])
            .allowed_methods(&[
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .trust_forwarded_host(true)
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "GET /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         X-Forwarded-Host: api.example.com\r\n\
         X-Forwarded-Port: 443\r\n\
         Origin: https://api.example.com\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (_status, body) = parse_response_parts(&resp);
    assert!(
        !body
            .to_ascii_lowercase()
            .contains("access-control-allow-origin:"),
        "same-origin behind forwarded host should not add ACAO: {body}"
    );
}

#[test]
fn http_cors_forwarded_rfc7239_same_origin() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://api.example.com"])
            .allowed_methods(&[
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .trust_forwarded_host(true)
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "GET /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Forwarded: proto=https;host=api.example.com\r\n\
         Origin: https://api.example.com\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (_status, body) = parse_response_parts(&resp);
    assert!(
        !body
            .to_ascii_lowercase()
            .contains("access-control-allow-origin:"),
        "same-origin with RFC 7239 Forwarded should not add ACAO: {body}"
    );
}

#[test]
fn http_cors_preflight_private_network_access_header() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::OPTIONS])
            .allow_private_network_access(true)
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    // Must target a route that registers OPTIONS (CORS runs after routing).
    let req = format!(
        "OPTIONS /users/1 HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\
         Access-Control-Request-Private-Network: true\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert_eq!(status, 200, "preflight body: {body}");
    assert!(
        body.contains("access-control-allow-private-network: true")
            || body.contains("Access-Control-Allow-Private-Network: true"),
        "expected ACA-PN on preflight: {body}"
    );
}

#[test]
fn http_cors_get_cross_origin_includes_aca_private_network_when_enabled() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::OPTIONS])
            .allow_private_network_access(true)
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "GET /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (_status, body) = parse_response_parts(&resp);
    assert!(
        body.contains("access-control-allow-private-network: true")
            || body.contains("Access-Control-Allow-Private-Network: true"),
        "expected ACA-PN on actual response when enabled: {body}"
    );
}

/// Preflight without credentials: security rejects before CORS short-circuit (401).
#[test]
fn http_cors_preflight_returns_401_without_api_key() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::HEAD, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /users/1 HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert_eq!(
        status, 401,
        "preflight without API key must fail auth first: {body}"
    );
}

/// Valid API key on preflight: 200 with CORS success headers (`docs/CORS_IMPLEMENTATION_AUDIT.md` §3).
#[test]
fn http_cors_preflight_with_api_key_returns_200_and_acao() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::HEAD, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /users/1 HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: GET\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert_eq!(status, 200, "preflight with API key: {body}");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("access-control-allow-origin: https://client.example"),
        "expected reflected ACAO: {body}"
    );
}

/// HTTP-level `Access-Control-Allow-Credentials` when enabled (non-wildcard origin + allowlist).
#[test]
fn http_cors_get_with_allow_credentials_includes_acac_and_reflected_origin() {
    let origin = "https://client.example";
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&[origin])
            .allowed_methods(&[Method::GET, Method::OPTIONS])
            .allow_credentials(true)
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "GET /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: {origin}\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (_status, body) = parse_response_parts(&resp);
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains(&format!("access-control-allow-origin: {origin}")),
        "expected reflected origin with credentials mode: {body}"
    );
    assert!(
        lower.contains("access-control-allow-credentials: true"),
        "expected ACAC when credentials enabled: {body}"
    );
}

#[test]
fn http_cors_idna_origin_exact_bytes_reflected() {
    // Allowlist and Origin use the same punycode serialization (no normalization).
    let origin = "https://xn--e28h.example";
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&[origin])
            .allowed_methods(&[Method::GET, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "GET /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: {origin}\r\n\
         X-API-Key: test123\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (_status, body) = parse_response_parts(&resp);
    assert!(
        body.contains(&format!("access-control-allow-origin: {origin}"))
            || body.contains(&format!("Access-Control-Allow-Origin: {origin}")),
        "expected reflected punycode origin: {body}"
    );
}

/// Preflight for a route that declares no `options` operation: answered from
/// the CORS policy of the actual method's route, without credentials (browsers
/// never send them on a preflight) and without running a handler.
#[test]
fn http_cors_synthesized_preflight_allows_listed_method() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::POST, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: POST\r\n\
         Access-Control-Request-Headers: content-type\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert!(
        status == 200 || status == 204,
        "synthesized preflight should succeed: {body}"
    );
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("access-control-allow-origin: https://client.example"),
        "expected ACAO: {body}"
    );
    assert!(
        lower.contains("access-control-allow-methods:"),
        "expected ACAM: {body}"
    );
}

#[test]
fn http_cors_synthesized_preflight_denies_unlisted_method() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: POST\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert_eq!(status, 403, "method not allowed by policy: {body}");
}

#[test]
fn http_cors_synthesized_preflight_denies_unlisted_origin() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::POST, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://evil.example\r\n\
         Access-Control-Request-Method: POST\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert_eq!(status, 403, "origin not allowed: {body}");
}

/// No route for the requested method, or no Access-Control-Request-Method at
/// all: still a routing miss.
#[test]
fn http_cors_options_without_matching_route_is_404() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    for extra in ["Access-Control-Request-Method: DELETE\r\n", ""] {
        let req = format!(
            "OPTIONS /pets HTTP/1.1\r\n\
             Host: 127.0.0.1:{port}\r\n\
             Origin: https://client.example\r\n\
             {extra}\r\n"
        );
        let resp = send_request(&f.addr(), &req);
        let (status, body) = parse_response_parts(&resp);
        assert_eq!(status, 404, "{extra:?}: {body}");
    }
}

/// A synthesized preflight runs the full middleware lifecycle: MetricsMiddleware
/// (registered before CORS) sees `after` too, so the active-request gauge does
/// not leak and the request is counted.
#[test]
fn http_cors_synthesized_preflight_closes_metrics() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::POST, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let f = CorsHttpFixture::new(cors);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: POST\r\n\r\n"
    );
    for _ in 0..3 {
        let resp = send_request(&f.addr(), &req);
        let (status, body) = parse_response_parts(&resp);
        assert!(status == 200 || status == 204, "{body}");
    }
    let metrics = send_request(
        &f.addr(),
        &format!("GET /metrics HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    // The preflights left nothing active; the only active request is this GET /metrics itself.
    assert!(
        !metrics.contains("http_server_active_requests{http_request_method=\"OPTIONS\"}"),
        "{metrics}"
    );
    assert!(
        metrics.contains("http_server_active_requests{http_request_method=\"GET\"} 1"),
        "{metrics}"
    );
}

/// Records whether a middleware registered after CORS was entered.
struct EnteredProbe {
    before: std::sync::atomic::AtomicUsize,
    after: std::sync::atomic::AtomicUsize,
}

impl brrtrouter::middleware::Middleware for EnteredProbe {
    fn before(
        &self,
        _req: &brrtrouter::dispatcher::HandlerRequest,
    ) -> Option<brrtrouter::dispatcher::HandlerResponse> {
        self.before
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        None
    }

    fn after(
        &self,
        _req: &brrtrouter::dispatcher::HandlerRequest,
        _res: &mut brrtrouter::dispatcher::HandlerResponse,
        _latency: std::time::Duration,
    ) {
        self.after.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Middleware behind CORS (rate limiting in production) is not entered by a
/// synthesized preflight that CORS answered: it spends no rate-limit budget and
/// gets no unmatched `after`.
#[test]
fn http_cors_synthesized_preflight_stops_at_cors() {
    let cors = Arc::new(
        CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://client.example"])
            .allowed_methods(&[Method::GET, Method::POST, Method::OPTIONS])
            .build()
            .unwrap(),
    );
    let probe = Arc::new(EnteredProbe {
        before: std::sync::atomic::AtomicUsize::new(0),
        after: std::sync::atomic::AtomicUsize::new(0),
    });
    let f = CorsHttpFixture::with_after_cors(cors, vec![probe.clone()]);
    let port = f.addr().port();
    let req = format!(
        "OPTIONS /pets HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Origin: https://client.example\r\n\
         Access-Control-Request-Method: POST\r\n\r\n"
    );
    let resp = send_request(&f.addr(), &req);
    let (status, body) = parse_response_parts(&resp);
    assert!(status == 200 || status == 204, "{body}");
    assert_eq!(probe.before.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(probe.after.load(std::sync::atomic::Ordering::SeqCst), 0);
}
