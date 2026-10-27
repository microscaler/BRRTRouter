//! With the client spans filtered out (`brrtrouter::http_client` and `may_minihttp` off), outbound
//! calls emit no client spans and add no `traceparent` of their own: a fetch sends none at all.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use tracing::span::{Attributes, Id};
use tracing::Subscriber;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

#[derive(Clone, Default)]
struct Names(Arc<Mutex<Vec<String>>>);

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Names {
    fn on_new_span(&self, attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
        self.0
            .lock()
            .unwrap()
            .push(attrs.metadata().name().to_string());
    }
}

#[test]
fn filtered_out_client_spans_cost_no_span_and_no_header() {
    let names = Names::default();
    let filter = Targets::new()
        .with_default(LevelFilter::TRACE)
        .with_target("brrtrouter::http_client", LevelFilter::OFF)
        .with_target("may_minihttp", LevelFilter::OFF);
    tracing_subscriber::registry()
        .with(names.clone().with_filter(filter))
        .try_init()
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        String::from_utf8(head).unwrap().to_ascii_lowercase()
    });
    let caller = tracing::info_span!("caller");
    let (status, body) = may_tracing::with_span(caller, || {
        brrtrouter::http::fetch_get(
            &format!("http://127.0.0.1:{port}/x"),
            &brrtrouter::http::HttpFetchOptions::default(),
        )
    })
    .unwrap();
    assert_eq!((status, body.as_slice()), (200, &b"ok"[..]));
    let head = server.join().unwrap();
    assert!(!head.contains("traceparent"), "{head}");
    let names = names.0.lock().unwrap().clone();
    assert_eq!(names, ["caller"], "only the caller's span");
}
