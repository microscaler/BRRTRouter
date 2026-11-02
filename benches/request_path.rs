//! End-to-end cost of one request through the whole BRRTRouter HTTP path.
//!
//! Runs the pet_store example service in-process with the middleware chain a generated
//! service installs (metrics, CORS), an API-key provider and a JWKS bearer provider
//! (cache-hit path, as in steady state), then drives keep-alive connections with prebuilt
//! requests from plain OS threads. A counting global allocator gives **allocations and bytes
//! per request**; wall time gives **µs per request** (one connection at a time, so it is
//! latency, not throughput).
//!
//! The client side allocates nothing per request (prebuilt bytes, fixed read buffer), so the
//! counts are the server's: parse → route → auth → validation → dispatch → handler
//! (`TryFrom`, controller, `to_value`) → response validation → write → access log.
//!
//! ```text
//! cargo bench --bench request_path              # all scenarios
//! BRRTR_BENCH_N=50000 cargo bench --bench request_path -- secure
//! BRRTR_BENCH_LOG=off cargo bench --bench request_path   # without the INFO access log
//! BRRTR_BENCH_OTEL=1 cargo bench --bench request_path     # with the OpenTelemetry layer
//! ```
//!
//! `bff_proxy_*` run a second, BFF-shaped service whose routes proxy (`x-service`) to an in-process
//! downstream answering a 20 KB / 200 KB JSON array: the cost a BFF adds to a proxied call. The
//! downstream keeps connections alive, as a BRRTRouter service does, so the proxy pool's reuse is
//! part of what is measured (`BRRTR_PROXY_POOL=off` measures a connection per call).
//!
//! Results are printed as a table and as one JSON line per scenario (prefix `JSON `) so two
//! runs (before/after a change) can be compared. See docs/perf/copy-and-borrow-audit.md.

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest};
use brrtrouter::middleware::{CorsMiddlewareBuilder, MemoryMiddleware, MetricsMiddleware};
use brrtrouter::router::Router;
use brrtrouter::security::{JwksBearerProvider, SecurityProvider, SecurityRequest};
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
/// Allocations of at least 64 KiB (counted separately: one of them dwarfs hundreds of small ones).
static LARGE: AtomicU64 = AtomicU64::new(0);
/// Diagnostics: `BRRTR_BENCH_TRACE=<min bytes>[,<count>]` prints the backtraces of the first
/// `count` (default 8) allocations of at least `min bytes` once the measured phase starts.
static TRACE_MIN: AtomicU64 = AtomicU64::new(u64::MAX);
static TRACE_LEFT: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static IN_TRACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Diagnostics: `BRRTR_BENCH_SITES=<every>` samples every `every`-th allocation of the measured
/// phase and prints the most frequent call sites (a few frames above the allocator).
static SITE_EVERY: AtomicU64 = AtomicU64::new(0);
static SITES: std::sync::Mutex<Option<std::collections::HashMap<String, u64>>> =
    std::sync::Mutex::new(None);

fn sample_site() {
    let bt = std::backtrace::Backtrace::force_capture().to_string();
    let frames: Vec<&str> = bt
        .lines()
        .filter_map(|l| l.trim().split_once(": ").map(|(_, f)| f))
        .filter(|f| {
            !(f.starts_with("std::")
                || f.starts_with("core::")
                || f.starts_with("alloc::")
                || f.starts_with("<alloc")
                || f.starts_with("<core")
                || f.starts_with("<std")
                || f.starts_with("request_path::")
                || f.starts_with("__rust")
                || f.starts_with("<smallvec")
                || f.starts_with("smallvec")
                || f.starts_with("<serde")
                || f.starts_with("serde")
                || f.starts_with("<&")
                || f.starts_with("<T as"))
        })
        .take(3)
        .collect();
    let key = frames.join("  <-  ");
    if let Ok(mut m) = SITES.lock() {
        *m.get_or_insert_with(Default::default)
            .entry(key)
            .or_insert(0) += 1;
    }
}

fn note(size: usize) {
    let n = ALLOCS.fetch_add(1, Ordering::Relaxed);
    let every = SITE_EVERY.load(Ordering::Relaxed);
    if every > 0 && n % every == 0 {
        let _ = IN_TRACE.try_with(|t| {
            if !t.replace(true) {
                sample_site();
                t.set(false);
            }
        });
    }
    BYTES.fetch_add(size as u64, Ordering::Relaxed);
    if size >= 64 << 10 {
        LARGE.fetch_add(1, Ordering::Relaxed);
    }
    if size as u64 >= TRACE_MIN.load(Ordering::Relaxed) {
        let _ = IN_TRACE.try_with(|t| {
            if !t.replace(true) {
                if TRACE_LEFT
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                    .is_ok()
                {
                    eprintln!(
                        "allocation of {size} bytes:\n{}",
                        std::backtrace::Backtrace::force_capture()
                    );
                }
                t.set(false);
            }
        });
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        System.alloc_zeroed(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const API_KEY: &str = "test123";
const SECRET: &[u8] = b"request-path-bench-secret-0123456789";

/// API-key check as the generated services' static provider does it.
struct StaticApiKey;
impl SecurityProvider for StaticApiKey {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } if location == "header" => req
                .get_header(&name.to_ascii_lowercase())
                .is_some_and(|v| v == API_KEY),
            _ => false,
        }
    }
}

/// A one-key JWKS (HS256 `oct`) served over plain HTTP from a thread.
fn jwks_server() -> String {
    use base64::Engine;
    let k = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(SECRET);
    let body = serde_json::json!({"keys": [{"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}]})
        .to_string();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let r = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = s.write_all(r.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}/.well-known/jwks.json")
}

fn token() -> String {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let mut h = Header::new(Algorithm::HS256);
    h.kid = Some("k1".into());
    h.typ = Some("at+jwt".into());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = serde_json::json!({
        "iss": "https://issuer.example", "aud": "bench", "sub": "user_01JBENCH",
        "exp": now + 7200, "iat": now, "scope": "read write",
        "sid": "ses_01JBENCH", "client_id": "web-portal", "tenant_id": "t_01JBENCH",
        "org_id": "org_01JBENCH", "roles": ["trader", "viewer"], "jti": "tok_01JBENCH"
    });
    encode(&h, &claims, &EncodingKey::from_secret(SECRET)).unwrap()
}

fn start() -> (ServerHandle, SocketAddr) {
    // Connection coroutines get may's configured stack. Services size it from BRRTR_STACK_SIZE
    // (PW: 16 MiB); 2 MiB is ample here, including the INFO subscriber's formatting.
    may::config().set_stack_size(2 * 1024 * 1024 / std::mem::size_of::<usize>());
    let spec = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/pet_store/doc/openapi.yaml"
    );
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(spec).expect("spec");
    let metrics = Arc::new(MetricsMiddleware::new());
    let memory = Arc::new(MemoryMiddleware::new());
    let mut dispatcher = Dispatcher::new();
    // As the generated main.rs: metrics and CORS are middleware; the memory middleware is only
    // handed to the service for /metrics (its background monitor samples it every 10 s).
    dispatcher.add_middleware(metrics.clone());
    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://app.example"])
        .build()
        .expect("cors");
    dispatcher.add_middleware(Arc::new(cors));
    unsafe { pet_store::registry::register_from_spec(&mut dispatcher, &routes) };
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        schemes,
        spec.into(),
        None,
        None,
    );
    service.set_metrics_middleware(metrics);
    service.set_memory_middleware(memory);
    // Every other scheme gets the API-key provider (which refuses anything but a header key).
    let names: Vec<String> = service.security_schemes.keys().cloned().collect();
    for name in names {
        service.register_security_provider(&name, Arc::new(StaticApiKey));
    }
    let jwks = JwksBearerProvider::new(jwks_server())
        .issuer("https://issuer.example".to_string())
        .audience("bench".to_string());
    service.register_security_provider("BearerAuth", Arc::new(jwks));
    service.resolve_security(&routes);
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    let handle = HttpServer(service).start(addr).expect("server");
    for _ in 0..200 {
        if TcpStream::connect(addr).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (handle, addr)
}

/// A proxied JSON array of `n` quote-like rows (≈100 bytes each), as a PW BFF route returns.
fn rows_json(n: usize) -> String {
    let rows: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            serde_json::json!({
                "symbol": format!("SYM{i:05}"), "price": 100.0 + i as f64 / 7.0,
                "size": i * 3, "ts": "2026-10-26T13:45:00.123Z", "tags": ["opt", "spx"]
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap()
}

/// The downstream Service a BFF proxies to: answers `/data/<rows>` with a prebuilt JSON array
/// and closes, as one request per connection (no allocation per request on this side).
fn downstream() -> u16 {
    let bodies: Vec<(String, Vec<u8>)> = [200usize, 2000]
        .iter()
        .map(|&n| {
            let body = rows_json(n);
            // Kept alive, as a BRRTRouter downstream is: the BFF's pool reuses the connection.
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            (
                format!("/data/{n} "),
                [head.into_bytes(), body.into_bytes()].concat(),
            )
        })
        .collect();
    let bodies = std::sync::Arc::new(bodies);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let bodies = std::sync::Arc::clone(&bodies);
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    // bodyless GETs: one head per request
                    let mut n = 0;
                    while !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                        match s.read(&mut buf[n..]) {
                            Ok(0) | Err(_) => return,
                            Ok(m) => n += m,
                        }
                    }
                    let line = &buf[..n];
                    match bodies
                        .iter()
                        .find(|(p, _)| line.windows(p.len()).any(|w| w == p.as_bytes()))
                    {
                        Some((_, rsp)) => {
                            if s.write_all(rsp).is_err() {
                                return;
                            }
                        }
                        None => return,
                    }
                }
            });
        }
    });
    port
}

/// A BFF: two proxied routes (`x-service`) whose 200 response schema is the rows array, with the
/// same middleware as `start`. Its handlers are what the generated untyped proxy controller is.
fn start_bff(downstream_port: u16) -> (ServerHandle, SocketAddr) {
    std::env::set_var("SERVICE_HTTP_PORT", downstream_port.to_string());
    std::env::remove_var("POD_NAMESPACE");
    let rows = r#"{"type":"array","items":{"type":"object","required":["symbol","price","size","ts"],"properties":{"symbol":{"type":"string"},"price":{"type":"number"},"size":{"type":"integer"},"ts":{"type":"string","format":"date-time"},"tags":{"type":"array","items":{"type":"string"}}}}}"#;
    let op = |n: usize| {
        format!(
            r#""/bff/rows{n}":{{"get":{{"operationId":"rows{n}","x-service":"127.0.0.1","x-brrtrouter-downstream-path":"/data/{n}","responses":{{"200":{{"description":"rows","content":{{"application/json":{{"schema":{rows}}}}}}}}}}}}}"#
        )
    };
    let spec = format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"bench bff","version":"1"}},"paths":{{{},{}}}}}"#,
        op(200),
        op(2000)
    );
    let dir = std::env::temp_dir().join(format!("brrtr-bench-bff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("openapi.json");
    std::fs::write(&path, spec).unwrap();
    let path = path.to_str().unwrap().to_string();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(&path).expect("bff spec");
    let metrics = Arc::new(MetricsMiddleware::new());
    let mut dispatcher = Dispatcher::new();
    dispatcher.add_middleware(metrics.clone());
    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://app.example"])
        .build()
        .expect("cors");
    dispatcher.add_middleware(Arc::new(cors));
    for r in &routes {
        let target = r.x_brrtrouter_downstream_path.clone().unwrap();
        unsafe {
            dispatcher.register_handler(&r.handler_name, move |req: HandlerRequest| {
                let rsp = brrtrouter::http::proxy_untyped(&req, "127.0.0.1", &target);
                req.reply(rsp);
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
    service.set_metrics_middleware(metrics);
    service.resolve_security(&routes);
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    let handle = HttpServer(service).start(addr).expect("bff server");
    for _ in 0..200 {
        if TcpStream::connect(addr).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (handle, addr)
}

/// Browser-like header set (what a BFF or the portal sends), plus scenario headers.
fn request(method: &str, target: &str, extra: &[(&str, &str)], body: Option<&str>) -> Vec<u8> {
    let mut r = format!("{method} {target} HTTP/1.1\r\n");
    for (k, v) in [
        ("Host", "pet-store.pw.svc.cluster.local:8080"),
        ("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0 Safari/537.36"),
        ("Accept", "application/json, text/plain, */*"),
        ("Accept-Encoding", "gzip, deflate, br"),
        ("Accept-Language", "en-GB,en;q=0.9"),
        ("Origin", "https://app.example"),
        ("Traceparent", "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        ("X-Request-Id", "01JBENCHREQUEST000000000000"),
        ("X-Tenant-Id", "t_01JBENCH"),
        ("Cookie", "pw_theme=dark; pw_locale=en-GB"),
    ] {
        r.push_str(&format!("{k}: {v}\r\n"));
    }
    for (k, v) in extra {
        r.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        r.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            b.len()
        ));
    }
    r.push_str("\r\n");
    if let Some(b) = body {
        r.push_str(b);
    }
    r.into_bytes()
}

/// Send one request and read exactly one response (Content-Length framed). No allocation.
fn roundtrip(s: &mut TcpStream, req: &[u8], buf: &mut [u8]) -> u16 {
    s.write_all(req).unwrap();
    let mut n = 0;
    loop {
        let m = s.read(&mut buf[n..]).unwrap_or_else(|e| {
            panic!(
                "read failed: {e}; got so far: {:?}",
                String::from_utf8_lossy(&buf[..n])
            )
        });
        assert!(
            m > 0,
            "server closed the connection: {:?}",
            String::from_utf8_lossy(&buf[..n])
        );
        n += m;
        let Some(end) = buf[..n].windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let head = &buf[..end];
        let status = std::str::from_utf8(&head[9..12]).unwrap().parse().unwrap();
        let mut cl = 0usize;
        for line in head.split(|b| *b == b'\n') {
            let line = std::str::from_utf8(line).unwrap().trim();
            if let Some((k, v)) = line.split_once(':') {
                if k.eq_ignore_ascii_case("content-length") {
                    cl = v.trim().parse().unwrap();
                }
            }
        }
        while n < end + 4 + cl {
            let m = s.read(&mut buf[n..]).unwrap();
            assert!(m > 0);
            n += m;
        }
        return status;
    }
}

struct Scenario {
    name: &'static str,
    req: Vec<u8>,
    expect: u16,
    /// Sent to the BFF (proxied routes) instead of the pet store.
    bff: bool,
}

/// Ends spans into nothing (the in-process cost of tracing without an exporter's).
#[derive(Debug)]
struct DropSpans;

impl opentelemetry_sdk::trace::SpanProcessor for DropSpans {
    fn on_start(&self, _span: &mut opentelemetry_sdk::trace::Span, _cx: &opentelemetry::Context) {}
    fn on_end(&self, span: opentelemetry_sdk::trace::SpanData) {
        drop(span);
    }
    fn force_flush(&self) -> Result<(), opentelemetry_sdk::error::OTelSdkError> {
        Ok(())
    }
    fn shutdown_with_timeout(
        &self,
        _timeout: Duration,
    ) -> Result<(), opentelemetry_sdk::error::OTelSdkError> {
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Test runners list a harness-less target with `--list` (nextest, `cargo test --all-targets`):
    // it has no unit tests to list.
    if args.iter().any(|a| a == "--list") {
        return;
    }
    // `cargo bench` passes `--bench`; anything else (a test run of all targets) gets a short
    // smoke run that only checks every scenario answers as expected.
    let bench = args.iter().any(|a| a == "--bench");
    let filter: Option<String> = args.iter().find(|a| !a.starts_with('-')).cloned();
    let n: u64 = std::env::var("BRRTR_BENCH_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if bench { 20_000 } else { 200 });
    let rounds = if bench { 5 } else { 1 };
    // Production services log at INFO (the access log, auth decisions). Format those events
    // as JSON into a sink so their cost is counted; BRRTR_BENCH_LOG=off measures without.
    let log = std::env::var("BRRTR_BENCH_LOG").unwrap_or_else(|_| "info".into());
    // BRRTR_BENCH_OTEL=1 adds the OpenTelemetry layer, as a service with OTLP configured has it:
    // every span (server, client, may_minihttp's) is built and ended into a processor that drops
    // it, so the cost counted is the in-process cost, not the exporter's.
    let otel = std::env::var("BRRTR_BENCH_OTEL").is_ok_and(|v| v != "0" && v != "off");
    if log != "off" || otel {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        let filter = tracing_subscriber::EnvFilter::new(if log == "off" { "info" } else { &log });
        let fmt = (log != "off").then(|| {
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(std::io::sink)
        });
        let otel = otel.then(|| {
            use opentelemetry::trace::TracerProvider as _;
            let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                .with_span_processor(DropSpans)
                .build();
            let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("bench"));
            std::mem::forget(provider);
            layer
        });
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt)
            .with(otel)
            .init();
    }
    let (app, addr) = start();
    let bearer = format!("Bearer {}", token());
    let key = [("X-API-Key", API_KEY)];
    let scenarios = vec![
        Scenario {
            name: "get_pet",
            req: request("GET", "/pets/12345", &key, None),
            expect: 200,
            bff: false,
        },
        Scenario {
            name: "list_pets_query",
            req: request("GET", "/pets?limit=10&offset=0", &key, None),
            expect: 200,
            bff: false,
        },
        Scenario {
            name: "get_post_2_params",
            req: request("GET", "/users/abc-123/posts/post1", &key, None),
            expect: 200,
            bff: false,
        },
        Scenario {
            name: "add_pet_json_body",
            req: request("POST", "/pets", &key, Some(r#"{"name":"Bella"}"#)),
            expect: 200,
            bff: false,
        },
        Scenario {
            name: "secure_jwt",
            req: request(
                "GET",
                "/secure",
                &[("Authorization", bearer.as_str())],
                None,
            ),
            expect: 200,
            bff: false,
        },
        Scenario {
            name: "bff_proxy_20k",
            req: request("GET", "/bff/rows200", &[], None),
            expect: 200,
            bff: true,
        },
        Scenario {
            name: "bff_proxy_200k",
            req: request("GET", "/bff/rows2000", &[], None),
            expect: 200,
            bff: true,
        },
    ];
    let (bff_app, bff_addr) = start_bff(downstream());
    // JWKS fetch happens in the background at provider start: give it a moment.
    std::thread::sleep(Duration::from_millis(500));
    println!(
        "{:<20} {:>12} {:>14} {:>12} {:>10}",
        "scenario", "allocs/req", "bytes/req", "µs/req", ">=64KiB"
    );
    for sc in &scenarios {
        if filter.as_deref().is_some_and(|f| !sc.name.contains(f)) {
            continue;
        }
        eprintln!("scenario {} ...", sc.name);
        let mut s = TcpStream::connect(if sc.bff { bff_addr } else { addr }).unwrap();
        s.set_nodelay(true).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut buf = vec![0u8; 1 << 20];
        let warmup = if bench { 2_000 } else { 20 };
        for _ in 0..warmup {
            let st = roundtrip(&mut s, &sc.req, &mut buf);
            assert_eq!(st, sc.expect, "{}: unexpected status {st}", sc.name);
        }
        if let Ok(spec) = std::env::var("BRRTR_BENCH_TRACE") {
            let mut it = spec.split(',');
            let min = it.next().and_then(|v| v.parse().ok()).unwrap_or(u64::MAX);
            let count = it.next().and_then(|v| v.parse().ok()).unwrap_or(8);
            TRACE_LEFT.store(count, Ordering::Relaxed);
            TRACE_MIN.store(min, Ordering::Relaxed);
        }
        if let Some(every) = std::env::var("BRRTR_BENCH_SITES")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            if let Ok(mut m) = SITES.lock() {
                *m = Some(Default::default());
            }
            SITE_EVERY.store(every, Ordering::Relaxed);
        }
        let (mut allocs, mut bytes, mut micros, mut large) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for _ in 0..rounds {
            let (a0, b0, l0) = (
                ALLOCS.load(Ordering::Relaxed),
                BYTES.load(Ordering::Relaxed),
                LARGE.load(Ordering::Relaxed),
            );
            let t = Instant::now();
            // Count (no formatting in the timed loop) and fail after it: an error answer must
            // not be measured as the scenario.
            let mut unexpected = 0u64;
            for _ in 0..n {
                if roundtrip(&mut s, &sc.req, &mut buf) != sc.expect {
                    unexpected += 1;
                }
            }
            let el = t.elapsed();
            assert_eq!(
                unexpected, 0,
                "{}: {unexpected} of {n} measured requests did not answer {}",
                sc.name, sc.expect
            );
            allocs.push((ALLOCS.load(Ordering::Relaxed) - a0) as f64 / n as f64);
            bytes.push((BYTES.load(Ordering::Relaxed) - b0) as f64 / n as f64);
            micros.push(el.as_secs_f64() * 1e6 / n as f64);
            large.push((LARGE.load(Ordering::Relaxed) - l0) as f64 / n as f64);
        }
        let med = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        SITE_EVERY.store(0, Ordering::Relaxed);
        if let Some(sites) = SITES.lock().ok().and_then(|mut m| m.take()) {
            let total: u64 = sites.values().sum();
            let mut v: Vec<_> = sites.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            eprintln!("allocation sites ({total} samples):");
            for (k, c) in v.iter().take(60) {
                eprintln!("{:6.1}%  {k}", 100.0 * *c as f64 / total as f64);
            }
        }
        let (a, b, m, l) = (
            med(&mut allocs),
            med(&mut bytes),
            med(&mut micros),
            med(&mut large),
        );
        println!(
            "{:<20} {:>12.1} {:>14.0} {:>12.2} {:>10.3}",
            sc.name, a, b, m, l
        );
        println!(
            "JSON {{\"scenario\":\"{}\",\"allocs_per_req\":{a:.2},\"bytes_per_req\":{b:.0},\"us_per_req\":{m:.3},\"large_per_req\":{l:.3},\"n\":{n}}}",
            sc.name
        );
    }
    drop(app);
    drop(bff_app);
}
