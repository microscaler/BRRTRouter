//! Idle CPU of a BRRTRouter service, and its latency under load.
//!
//! Starts the pet_store service as a generated main does (`RuntimeConfig::from_env`: may workers,
//! stack, pool and poll timeout; metrics + CORS middleware), sends nothing for `BRRTR_IDLE_SECS`
//! (default 20) and reports this process's CPU in mcores. With `BRRTR_LOAD_CONNS=<n>` it then drives
//! `n` keep-alive connections (`GET /pets/12345`, `BRRTR_LOAD_GAP_US` between requests per
//! connection) for `BRRTR_LOAD_SECS` and reports throughput, latency percentiles and CPU.
//!
//! ```text
//! cargo bench --bench idle_cpu
//! BRRTR_MAY_POLL_TIMEOUT_MS=10 cargo bench --bench idle_cpu        # may's own default
//! BRRTR_LOAD_CONNS=64 BRRTR_LOAD_GAP_US=2000 cargo bench --bench idle_cpu
//! ```
use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::middleware::{CorsMiddlewareBuilder, MetricsMiddleware};
use brrtrouter::router::Router;
use brrtrouter::security::{SecurityProvider, SecurityRequest};
use brrtrouter::server::{AppService, HttpServer};
use brrtrouter::spec::SecurityScheme;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn cpu_ticks() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap();
    let rest = &s[s.rfind(')').unwrap() + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    (f[11].parse().unwrap(), f[12].parse().unwrap()) // utime, stime (fields 14, 15)
}

/// API-key check as the generated services' static provider does it (pet_store's routes need one).
struct StaticApiKey;
impl SecurityProvider for StaticApiKey {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } if location == "header" => req
                .get_header(&name.to_ascii_lowercase())
                .is_some_and(|v| v == "test123"),
            _ => false,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Test runners list a harness-less target with `--list`: it has no unit tests to list.
    if args.iter().any(|a| a == "--list") {
        return;
    }
    // CPU time comes from /proc (Linux); elsewhere there is nothing to measure.
    if !std::path::Path::new("/proc/self/stat").exists() {
        println!("idle_cpu: skipped (needs /proc/self/stat, i.e. Linux)");
        return;
    }
    // Without `--bench` (a test run of all targets): a 1 s smoke run.
    let bench = args.iter().any(|a| a == "--bench");
    let cfg = brrtrouter::runtime_config::RuntimeConfig::from_env();
    may::config().set_stack_size(cfg.stack_size);
    may::config().set_workers(cfg.may_workers);
    let spec = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/pet_store/doc/openapi.yaml"
    );
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(spec).expect("spec");
    let metrics = Arc::new(MetricsMiddleware::new());
    let mut dispatcher = Dispatcher::new();
    dispatcher.add_middleware(metrics.clone());
    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://app.example"])
        .build()
        .unwrap();
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
    let names: Vec<String> = service.security_schemes.keys().cloned().collect();
    for name in names {
        service.register_security_provider(&name, Arc::new(StaticApiKey));
    }
    service.resolve_security(&routes);
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let _h = HttpServer(service).start(addr).expect("server");
    std::thread::sleep(Duration::from_secs(if bench { 3 } else { 1 }));
    let secs: u64 = std::env::var("BRRTR_IDLE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if bench { 20 } else { 1 });
    let hz = 100.0; // USER_HZ
    let (u0, s0) = cpu_ticks();
    let t = Instant::now();
    std::thread::sleep(Duration::from_secs(secs));
    let (u1, s1) = cpu_ticks();
    let el = t.elapsed().as_secs_f64();
    let threads = std::fs::read_dir("/proc/self/task").unwrap().count();
    println!(
        "workers={} poll_ms={} threads={threads} idle_mcores={:.1} (user {:.1}, sys {:.1})",
        cfg.may_workers,
        may::config().get_timeout_ns() / 1_000_000,
        1000.0 * ((u1 - u0) + (s1 - s0)) as f64 / hz / el,
        1000.0 * (u1 - u0) as f64 / hz / el,
        1000.0 * (s1 - s0) as f64 / hz / el
    );
    // Load phase: BRRTR_LOAD_CONNS keep-alive connections, each sending GET /pets/1 back to back
    // (with BRRTR_LOAD_GAP_US between requests) for BRRTR_LOAD_SECS; latency percentiles.
    let conns: usize = std::env::var("BRRTR_LOAD_CONNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if conns > 0 && bench {
        let lsecs: u64 = std::env::var("BRRTR_LOAD_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let gap: u64 = std::env::var("BRRTR_LOAD_GAP_US")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let req = b"GET /pets/12345 HTTP/1.1\r\nHost: x\r\nX-API-Key: test123\r\n\r\n".to_vec();
        let deadline = Instant::now() + Duration::from_secs(lsecs);
        let (cu0, cs0) = cpu_ticks();
        let t0 = Instant::now();
        let hs: Vec<_> = (0..conns)
            .map(|_| {
                let req = req.clone();
                std::thread::spawn(move || {
                    use std::io::{Read, Write};
                    let mut s = std::net::TcpStream::connect(addr).unwrap();
                    s.set_nodelay(true).unwrap();
                    // A response that never completes fails the run instead of hanging it.
                    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                    s.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
                    let mut buf = vec![0u8; 65536];
                    let mut lat = Vec::new();
                    while Instant::now() < deadline {
                        let t = Instant::now();
                        s.write_all(&req).unwrap();
                        let mut n = 0;
                        let status = loop {
                            let m = s.read(&mut buf[n..]).unwrap();
                            assert!(m > 0);
                            n += m;
                            if let Some(end) = buf[..n].windows(4).position(|w| w == b"\r\n\r\n") {
                                let head = std::str::from_utf8(&buf[..end])
                                    .unwrap()
                                    .to_ascii_lowercase();
                                let cl: usize = head
                                    .lines()
                                    .find_map(|l| l.strip_prefix("content-length:"))
                                    .map(|v| v.trim().parse().unwrap())
                                    .unwrap_or(0);
                                if n >= end + 4 + cl {
                                    break head[9..12].to_string();
                                }
                            }
                        };
                        // Only answered requests are measured: an error answer fails the run.
                        assert_eq!(status, "200", "GET /pets/12345 answered {status}");
                        lat.push(t.elapsed().as_micros() as u64);
                        if gap > 0 {
                            std::thread::sleep(Duration::from_micros(gap));
                        }
                    }
                    lat
                })
            })
            .collect();
        let mut all: Vec<u64> = hs.into_iter().flat_map(|h| h.join().unwrap()).collect();
        let el = t0.elapsed().as_secs_f64();
        let (cu1, cs1) = cpu_ticks();
        all.sort_unstable();
        let pct = |q: f64| all[((all.len() as f64 * q) as usize).min(all.len() - 1)];
        println!(
            "  load conns={conns} gap_us={gap} rps={:.0} p50={}us p99={}us p999={}us max={}us cpu_mcores={:.0}",
            all.len() as f64 / el, pct(0.5), pct(0.99), pct(0.999), all[all.len() - 1],
            1000.0 * ((cu1 - cu0) + (cs1 - cs0)) as f64 / hz / el
        );
    }
    std::process::exit(0);
}
