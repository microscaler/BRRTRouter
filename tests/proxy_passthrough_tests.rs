//! ADR 006: proxied JSON pass-through (`BRRTR_PROXY_PASSTHROUGH`, default off) and the response
//! validation mode (`BRRTR_RESPONSE_VALIDATION`, default full). Both are read once per process, so
//! each scenario runs this test binary again as a child with its environment.

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest};
use brrtrouter::router::Router;
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// Off-schema on purpose (`symbol` is a number), with spacing serde_json would not write.
const BAD: &str = r#"[ {"symbol": 1, "price": 2.50} ]"#;
const GOOD: &str = r#"[ {"symbol": "SPX", "price": 2.50} ]"#;
const NOT_FOUND: &str = r#"{"error":"nope"}"#;

fn downstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut buf = [0u8; 8192];
            let mut n = 0;
            while !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf[n..]) {
                    Ok(0) | Err(_) => break,
                    Ok(m) => n += m,
                }
            }
            let head = String::from_utf8_lossy(&buf[..n]);
            let (status, body) = if head.starts_with("GET /down/bad ") {
                ("200 OK", BAD)
            } else if head.starts_with("GET /down/good ") {
                ("200 OK", GOOD)
            } else {
                ("404 Not Found", NOT_FOUND)
            };
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    port
}

fn spec() -> String {
    let schema = r#"{"type":"array","items":{"type":"object","properties":{"symbol":{"type":"string"},"price":{"type":"number"}}}}"#;
    let op = |path: &str, id: &str, down: &str, validate: bool| {
        let flag = if validate {
            r#""x-brrtrouter-validate-response":true,"#
        } else {
            ""
        };
        format!(
            r#""{path}":{{"get":{{"operationId":"{id}","x-service":"127.0.0.1","x-brrtrouter-downstream-path":"{down}",{flag}"responses":{{"200":{{"description":"rows","content":{{"application/json":{{"schema":{schema}}}}}}},"404":{{"description":"missing","content":{{"application/json":{{"schema":{{"type":"object"}}}}}}}}}}}}}}"#
        )
    };
    format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"passthrough","version":"1"}},"paths":{{{},{},{},{}}}}}"#,
        op("/pass_bad", "pass_bad", "/down/bad", false),
        op("/check_bad", "check_bad", "/down/bad", true),
        op("/check_good", "check_good", "/down/good", true),
        op("/missing", "missing", "/down/missing", false),
    )
}

fn start() -> (ServerHandle, SocketAddr) {
    may::config().set_stack_size(2 * 1024 * 1024 / std::mem::size_of::<usize>());
    std::env::set_var("SERVICE_HTTP_PORT", downstream().to_string());
    std::env::remove_var("POD_NAMESPACE");
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join("openapi.json");
    std::fs::write(&path, spec()).unwrap();
    let path = path.to_str().unwrap().to_string();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(&path).expect("spec");
    let mut dispatcher = Dispatcher::new();
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
    service.set_metrics_middleware(Arc::new(brrtrouter::middleware::MetricsMiddleware::new()));
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

/// (status, lowercased head, body)
fn get(addr: SocketAddr, path: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: bff\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    // Read the head, then exactly Content-Length bytes (the server may keep the socket open).
    loop {
        let n = s.read(&mut buf).unwrap();
        assert!(n > 0, "closed early: {:?}", String::from_utf8_lossy(&out));
        out.extend_from_slice(&buf[..n]);
        let Some(end) = out.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&out[..end]).to_ascii_lowercase();
        let cl: usize = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .map(|v| v.trim().parse().unwrap())
            .unwrap_or(0);
        while out.len() < end + 4 + cl {
            let n = s.read(&mut buf).unwrap();
            assert!(n > 0);
            out.extend_from_slice(&buf[..n]);
        }
        let status = head[9..12].parse().unwrap();
        let body = String::from_utf8(out[end + 4..end + 4 + cl].to_vec()).unwrap();
        return (status, head, body);
    }
}

/// (status, body) of every route, then the `/metrics` text.
fn run_scenario() -> String {
    let (app, addr) = start();
    let mut out = String::new();
    for path in ["/pass_bad", "/check_bad", "/check_good", "/missing"] {
        let (status, head, body) = get(addr, path);
        assert!(!head.contains("x-brrtrouter-raw-encoding"), "{head}");
        out.push_str(&format!("{path} {status} {body}\n"));
    }
    let (_, _, metrics) = get(addr, "/metrics");
    out.push_str(&metrics);
    drop(app);
    out
}

/// Run `child_scenario` in a fresh process with `env`; returns its report.
fn scenario(env: &[(&str, &str)]) -> String {
    let exe = std::env::current_exe().unwrap();
    let mut cmd = std::process::Command::new(exe);
    cmd.args([
        "--exact",
        "child_scenario",
        "--nocapture",
        "--test-threads=1",
    ])
    .env("BRRTR_PT_CHILD", "1")
    .env_remove("BRRTR_PROXY_PASSTHROUGH")
    .env_remove("BRRTR_RESPONSE_VALIDATION");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let start = stdout.find("<<REPORT").expect("report") + "<<REPORT\n".len();
    let end = stdout.find("REPORT>>").expect("report end");
    stdout[start..end].to_string()
}

#[test]
fn child_scenario() {
    if std::env::var_os("BRRTR_PT_CHILD").is_none() {
        return;
    }
    println!("<<REPORT\n{}REPORT>>", run_scenario());
}

fn line<'a>(report: &'a str, path: &str) -> (u16, &'a str) {
    let l = report
        .lines()
        .find(|l| l.starts_with(&format!("{path} ")))
        .unwrap();
    let rest = &l[path.len() + 1..];
    (rest[..3].parse().unwrap(), &rest[4..])
}

#[test]
fn default_is_parsed_and_fully_validated() {
    let r = scenario(&[]);
    // Off-schema downstream bodies are refused, as always.
    assert_eq!(line(&r, "/pass_bad").0, 500, "{r}");
    assert_eq!(line(&r, "/check_bad").0, 500, "{r}");
    // A valid one is re-serialised (same JSON, not the same bytes).
    let (status, body) = line(&r, "/check_good");
    assert_eq!(status, 200, "{r}");
    assert_ne!(body, GOOD);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(body).unwrap(),
        serde_json::from_str::<serde_json::Value>(GOOD).unwrap()
    );
    assert_eq!(line(&r, "/missing").0, 404, "{r}");
    assert!(
        r.contains(
            "brrtrouter_response_validation_total{handler=\"pass_bad\",result=\"mismatch\"} 1"
        ),
        "{r}"
    );
}

#[test]
fn pass_through_with_full_validation_sends_the_downstream_bytes() {
    let r = scenario(&[("BRRTR_PROXY_PASSTHROUGH", "on")]);
    assert_eq!(line(&r, "/pass_bad").0, 500, "{r}");
    assert_eq!(line(&r, "/check_bad").0, 500, "{r}");
    assert_eq!(line(&r, "/check_good"), (200, GOOD), "{r}");
    let (status, body) = line(&r, "/missing");
    assert_eq!(status, 404, "{r}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(body).unwrap(),
        serde_json::json!({"error": "nope"})
    );
    assert!(
        r.contains("brrtrouter_response_validation_total{handler=\"check_good\",result=\"ok\"} 1"),
        "{r}"
    );
}

#[test]
fn validation_off_passes_bytes_unless_the_operation_forces_it() {
    let r = scenario(&[
        ("BRRTR_PROXY_PASSTHROUGH", "on"),
        ("BRRTR_RESPONSE_VALIDATION", "off"),
    ]);
    assert_eq!(line(&r, "/pass_bad"), (200, BAD), "{r}");
    // x-brrtrouter-validate-response: true is always checked.
    assert_eq!(line(&r, "/check_bad").0, 500, "{r}");
    assert_eq!(line(&r, "/check_good"), (200, GOOD), "{r}");
    assert!(
        r.contains("brrtrouter_response_validation_skipped_total{handler=\"pass_bad\"} 1"),
        "{r}"
    );
}

#[test]
fn sampled_mismatches_are_counted_not_failed() {
    let r = scenario(&[
        ("BRRTR_PROXY_PASSTHROUGH", "on"),
        ("BRRTR_RESPONSE_VALIDATION", "sample:0.99"),
    ]);
    // sample:0.99 checks the first response: a mismatch is counted and the bytes still go out.
    assert_eq!(line(&r, "/pass_bad"), (200, BAD), "{r}");
    assert!(
        r.contains(
            "brrtrouter_response_validation_total{handler=\"pass_bad\",result=\"mismatch\"} 1"
        ),
        "{r}"
    );
}
