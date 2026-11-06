//! Shared by the access-log mode tests: a pet store server and a subscriber that records every "Request completed"
//! event's fields and every `http_request` span's field names. Each test binary sets the `BRRTR_ACCESS_LOG*` and
//! `BRRTR_REQUEST_SPAN` variables before the first request, since the modes are read once per process.
#![allow(dead_code)]

use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService};
use pet_store::registry;
use std::collections::{BTreeMap, HashMap};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::Layer;

/// One "Request completed" event: field name → value as text.
pub type Fields = BTreeMap<String, String>;

#[derive(Default)]
pub struct Captured {
    pub completed: Vec<Fields>,
    pub span_fields: Vec<Vec<String>>,
}

pub fn captured() -> &'static Mutex<Captured> {
    static C: OnceLock<Mutex<Captured>> = OnceLock::new();
    C.get_or_init(Default::default)
}

struct Collect(Fields);

impl Visit for Collect {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
}

struct Capture;

impl<S: tracing::Subscriber> Layer<S> for Capture {
    fn on_event(&self, event: &tracing::Event<'_>, _cx: Context<'_, S>) {
        let mut c = Collect(Fields::new());
        event.record(&mut c);
        if c.0.get("message").map(String::as_str) == Some("Request completed")
            && c.0.contains_key("duration_ms")
        {
            captured().lock().unwrap().completed.push(c.0);
        }
    }
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _cx: Context<'_, S>,
    ) {
        if attrs.metadata().name() == "http_request" {
            let names = attrs
                .metadata()
                .fields()
                .iter()
                .map(|f| f.name().to_string())
                .collect();
            captured().lock().unwrap().span_fields.push(names);
        }
    }
}

/// Installs the capturing subscriber (process-wide: the server's coroutines run on other threads).
pub fn install() {
    let subscriber = tracing_subscriber::registry()
        .with(Capture.with_filter(tracing_subscriber::filter::LevelFilter::INFO));
    tracing::subscriber::set_global_default(subscriber).expect("one subscriber per test binary");
}

pub fn start() -> (ServerHandle, SocketAddr) {
    std::env::set_var("BRRTR_STACK_SIZE", "0x40000");
    let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    let (routes, _slug) = brrtrouter::load_spec("examples/openapi.yaml").unwrap();
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        HashMap::new(),
        PathBuf::from("examples/openapi.yaml"),
        None,
        None,
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (handle, addr)
}

/// Sends one request and returns the status code.
pub fn get(addr: SocketAddr, path: &str) -> u16 {
    let resp = crate::common::http::send_request(
        &addr,
        &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nX-Probe: 1\r\n\r\n"),
    );
    resp.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

/// The "Request completed" events for `path` once they have been logged (they are emitted when the request's logger
/// drops, after the response is written), waiting up to a second for `want` of them.
// Plain test threads, not coroutines: sleeping here blocks nothing the server runs on.
#[allow(clippy::disallowed_methods)]
pub fn completed_for(path: &str, want: usize) -> Vec<Fields> {
    for _ in 0..100 {
        let got: Vec<Fields> = captured()
            .lock()
            .unwrap()
            .completed
            .iter()
            .filter(|f| f.get("path").map(String::as_str) == Some(path))
            .cloned()
            .collect();
        if got.len() >= want {
            return got;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    captured()
        .lock()
        .unwrap()
        .completed
        .iter()
        .filter(|f| f.get("path").map(String::as_str) == Some(path))
        .cloned()
        .collect()
}
