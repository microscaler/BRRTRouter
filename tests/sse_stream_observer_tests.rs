#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]
//! End to end through the may_minihttp server: a client that leaves an event stream while nothing is being sent.
//!
//! Coverage and strategy:
//! - An in-process stream whose producer is silent: the server notices the client's closed socket between frames and
//!   drops the receiver, so the producer's sender reports the client gone (`SseSender::is_closed`).
//! - A relayed stream (`proxy_sse`) to a downstream that sends one frame and then nothing: the client drops its
//!   connection mid-stream and the registered `StreamObserver`'s lease closes once, as `client_closed`, within a few
//!   seconds rather than at the 300 s idle timeout. The observer also sees the request headers the stream was opened
//!   with.
//!
//! Each test runs a real server (the pet store spec, a request hook that answers the stream routes) and a raw TCP
//! client, so the disconnect is a real socket close. The observer is process-wide; it records only the handler
//! names these tests use.

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::http::stream_observer::{
    set_stream_observer, CloseReason, StreamLease, StreamObserver, StreamOpen,
};
use brrtrouter::router::Router;
use brrtrouter::server::{AppService, HttpServer, RequestHook, ServerHandle};
use brrtrouter::sse::SseSender;
use pet_store::registry;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod tracing_util;
use tracing_util::TestTracing;

struct Fixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.stop();
        }
    }
}

fn start(hook: RequestHook) -> Fixture {
    std::env::set_var("BRRTR_STACK_SIZE", "0x40000");
    let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    let tracing = TestTracing::init();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let mut service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        None,
        None,
    );
    service.set_request_hook(Some(hook));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    Fixture {
        _tracing: tracing,
        handle: Some(handle),
        addr,
    }
}

/// Open `path` as an event stream, read until `marker` arrives, then close the connection mid-stream.
fn open_then_leave(addr: SocketAddr, path: &str, extra: &str, marker: &str) {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n{extra}\r\n"
    )
    .unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 1024];
    while !String::from_utf8_lossy(&got).contains(marker) {
        let n = s.read(&mut buf).unwrap();
        assert!(
            n > 0,
            "stream ended early: {}",
            String::from_utf8_lossy(&got)
        );
        got.extend_from_slice(&buf[..n]);
    }
    drop(s);
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) -> Duration {
    let started = Instant::now();
    while !done() {
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    started.elapsed()
}

#[test]
fn the_server_drops_a_quiet_stream_whose_client_left() {
    let senders: Arc<Mutex<Vec<SseSender>>> = Arc::default();
    let held = Arc::clone(&senders);
    let f = start(Arc::new(move |ctx| {
        (ctx.path == "/quiet").then(|| {
            let (tx, rx) = brrtrouter::sse::channel();
            tx.send("hello");
            held.lock().unwrap().push(tx);
            HandlerResponse::sse_live(rx)
        })
    }));
    open_then_leave(f.addr, "/quiet", "", "data: hello");
    let tx = senders.lock().unwrap().pop().unwrap();
    // nothing is sent after the hello: only the probe can notice
    let took = wait_for("the receiver to be dropped", || tx.is_closed());
    assert!(took < Duration::from_secs(5), "{took:?}");
}

/// A downstream that answers with an event stream, sends `hello`, then stays silent with the connection open.
fn silent_downstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut conn) = conn else { return };
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 1024];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match conn.read(&mut tmp) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let frame = "event: hello\ndata: {}\n\n";
                let _ = write!(
                    conn,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{frame}\r\n",
                    frame.len()
                );
                let _ = conn.flush();
                // silent from here; hold the connection until the relay drops it
                let _ = conn.read(&mut tmp);
            });
        }
    });
    port
}

#[derive(Default)]
struct Seen {
    opened: Mutex<Vec<Option<String>>>,
    closed: Mutex<Vec<CloseReason>>,
}

struct Recorder(Arc<Seen>);
struct RecLease(Arc<Seen>);

impl StreamObserver for Recorder {
    fn opened(&self, open: &StreamOpen<'_>) -> Option<Box<dyn StreamLease>> {
        if open.handler() != "__e2e_relayed" {
            return None;
        }
        self.0
            .opened
            .lock()
            .unwrap()
            .push(open.header("x-client-tab").map(str::to_string));
        Some(Box::new(RecLease(Arc::clone(&self.0))))
    }
}

impl StreamLease for RecLease {
    fn closed(self: Box<Self>, reason: CloseReason) {
        self.0.closed.lock().unwrap().push(reason);
    }
}

#[test]
fn a_relayed_stream_left_mid_stream_closes_its_lease_as_client_closed() {
    let seen = Arc::new(Seen::default());
    assert!(set_stream_observer(Arc::new(Recorder(Arc::clone(&seen)))).is_ok());
    let port = silent_downstream();
    std::env::remove_var("POD_NAMESPACE");
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    let f = start(Arc::new(|ctx| {
        if ctx.path != "/relayed" {
            return None;
        }
        let mut headers = HeaderVec::new();
        for (k, v) in ctx.headers.iter() {
            headers.push((Arc::clone(k), v.clone()));
        }
        let (reply_tx, _reply_rx) = may::sync::mpsc::channel();
        let req = HandlerRequest {
            request_id: brrtrouter::ids::RequestId::new(),
            method: ctx.method.clone(),
            path: "/stream".to_string(),
            handler_name: "__e2e_relayed".to_string(),
            path_params: Default::default(),
            query_params: Default::default(),
            raw_query: None,
            headers,
            cookies: Default::default(),
            body: None,
            jwt_claims: None,
            reply_tx,
            queue_guard: None,
            span: tracing::Span::none(),
        };
        Some(brrtrouter::http::proxy_sse(&req, "127.0.0.1", "/stream"))
    }));
    open_then_leave(f.addr, "/relayed", "X-Client-Tab: t-1\r\n", "event: hello");
    assert_eq!(*seen.opened.lock().unwrap(), vec![Some("t-1".to_string())]);
    let took = wait_for("the lease to close", || {
        !seen.closed.lock().unwrap().is_empty()
    });
    assert!(took < Duration::from_secs(6), "{took:?}");
    assert_eq!(
        *seen.closed.lock().unwrap(),
        vec![CloseReason::ClientClosed]
    );
    // exactly once
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(seen.closed.lock().unwrap().len(), 1);
}
