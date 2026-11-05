//! `BRRTR_HANDLER_CONCURRENCY`: how many coroutines serve an untyped handler.
//!
//! With one coroutine a handler's requests are served in turn, so a handler that waits on another
//! service makes every request queued behind it wait too. With `n`, `n` of them run at once. The
//! default is two; `1` opts out. Each test uses its own handler name and the per-handler variable, so they do not race on
//! the process environment.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, unsafe_code)]

use brrtrouter::dispatcher::{
    handler_concurrency, spawn_untyped_with_stack_size_and_name, HandlerRequest, HandlerResponse,
    HandlerSender, HeaderVec, DEFAULT_HANDLER_CONCURRENCY,
};
use brrtrouter::ids::RequestId;
use brrtrouter::router::ParamVec;
use http::Method;
use may::sync::mpsc;
use std::collections::HashSet;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_millis(100);

fn request(handler: &str, reply_tx: mpsc::Sender<Box<HandlerResponse>>) -> HandlerRequest {
    HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/wait".to_string(),
        handler_name: handler.to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        raw_query: None,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
        queue_guard: None,
        span: tracing::Span::none(),
    }
}

/// A handler that waits like a call to another service, then answers which coroutine served it:
/// each coroutine has its own stack, so the address of a local tells them apart.
fn waiting_handler(_req: HandlerRequest) -> HandlerResponse {
    may::coroutine::sleep(WAIT);
    let marker = 0u8;
    let me = format!("{:p}", std::hint::black_box(&marker));
    HandlerResponse::json(200, serde_json::json!({ "coroutine": me }))
}

/// Send `n` requests at once; the time until all are answered, and the coroutines that answered.
fn burst(tx: &HandlerSender, handler: &str, n: usize) -> (Duration, HashSet<String>) {
    let start = Instant::now();
    let replies: Vec<_> = (0..n)
        .map(|_| {
            let (reply_tx, reply_rx) = mpsc::channel();
            tx.send(request(handler, reply_tx)).unwrap();
            reply_rx
        })
        .collect();
    let mut served_by = HashSet::new();
    for rx in replies {
        let resp = rx.recv().unwrap();
        assert_eq!(resp.status, 200);
        served_by.insert(resp.body["coroutine"].as_str().unwrap().to_string());
    }
    (start.elapsed(), served_by)
}

#[test]
fn two_coroutines_by_default() {
    let name = "concurrency_default_probe";
    assert_eq!(handler_concurrency(name), DEFAULT_HANDLER_CONCURRENCY);
    assert_eq!(DEFAULT_HANDLER_CONCURRENCY, 2);
    let tx =
        unsafe { spawn_untyped_with_stack_size_and_name(waiting_handler, 64 * 1024, Some(name)) }
            .unwrap();
    // Four requests on two coroutines: both serve, so at least two waits (no upper bound: a slow
    // scheduler must not fail correct code; served_by already shows the requests were shared).
    let (took, served_by) = burst(&tx, name, 4);
    assert_eq!(served_by.len(), 2);
    assert!(took >= WAIT * 2, "took {took:?}");
}

#[test]
fn one_opts_a_handler_out() {
    let name = "concurrency_one_probe";
    // SAFETY (test): only this test reads this handler's variable.
    unsafe { std::env::set_var("BRRTR_HANDLER_CONCURRENCY__CONCURRENCY_ONE_PROBE", "1") };
    assert_eq!(handler_concurrency(name), 1);
    let tx =
        unsafe { spawn_untyped_with_stack_size_and_name(waiting_handler, 64 * 1024, Some(name)) }
            .unwrap();
    let (took, served_by) = burst(&tx, name, 4);
    assert_eq!(served_by.len(), 1);
    assert!(
        took >= WAIT * 4,
        "4 requests in turn take 4 waits, took {took:?}"
    );
}

#[test]
fn per_handler_concurrency_serves_requests_at_once() {
    let name = "concurrency_four_probe";
    // SAFETY (test): only this test reads this handler's variable.
    unsafe { std::env::set_var("BRRTR_HANDLER_CONCURRENCY__CONCURRENCY_FOUR_PROBE", "4") };
    assert_eq!(handler_concurrency(name), 4);
    let tx =
        unsafe { spawn_untyped_with_stack_size_and_name(waiting_handler, 64 * 1024, Some(name)) }
            .unwrap();
    let (took, served_by) = burst(&tx, name, 4);
    assert_eq!(served_by.len(), 4, "each request on its own coroutine");
    assert!(
        took < WAIT * 3,
        "4 requests on 4 coroutines take about one wait, took {took:?}"
    );

    // Eight requests on four coroutines: two waits.
    let (took, served_by) = burst(&tx, name, 8);
    assert_eq!(served_by.len(), 4);
    assert!(took >= WAIT * 2, "took {took:?}");
}

#[test]
fn concurrency_is_clamped_and_bad_values_ignored() {
    // SAFETY (test): only this test reads these handlers' variables.
    unsafe {
        std::env::set_var("BRRTR_HANDLER_CONCURRENCY__CONCURRENCY_ZERO_PROBE", "0");
        std::env::set_var(
            "BRRTR_HANDLER_CONCURRENCY__CONCURRENCY_HUGE_PROBE",
            "100000",
        );
        std::env::set_var("BRRTR_HANDLER_CONCURRENCY__CONCURRENCY_BAD_PROBE", "lots");
    }
    assert_eq!(handler_concurrency("concurrency_zero_probe"), 1);
    assert_eq!(
        handler_concurrency("concurrency_huge_probe"),
        brrtrouter::dispatcher::MAX_HANDLER_CONCURRENCY
    );
    assert_eq!(
        handler_concurrency("concurrency_bad_probe"),
        DEFAULT_HANDLER_CONCURRENCY
    );
}

#[test]
fn a_panicking_request_does_not_stop_the_other_coroutines() {
    let name = "concurrency_panic_probe";
    // SAFETY (test): only this test reads this handler's variable.
    unsafe { std::env::set_var("BRRTR_HANDLER_CONCURRENCY__CONCURRENCY_PANIC_PROBE", "2") };
    let handler = |req: HandlerRequest| {
        if req.path == "/panic" {
            panic!("boom");
        }
        HandlerResponse::json(200, serde_json::json!({}))
    };
    let tx =
        unsafe { spawn_untyped_with_stack_size_and_name(handler, 64 * 1024, Some(name)) }.unwrap();
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut r = request(name, reply_tx);
    r.path = "/panic".to_string();
    tx.send(r).unwrap();
    assert_eq!(reply_rx.recv().unwrap().status, 500);
    for _ in 0..4 {
        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(request(name, reply_tx)).unwrap();
        assert_eq!(reply_rx.recv().unwrap().status, 200);
    }
}
