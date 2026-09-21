#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::{
    dispatcher::Dispatcher, load_spec, middleware::TracingMiddleware, router::Router,
};
use http::Method;
use pet_store::registry;
use std::sync::Arc;
mod tracing_util;
use tracing_util::TestTracing;

#[test]
fn test_tracing_middleware_emits_spans() {
    let mut tracing = TestTracing::init();

    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    // The server creates the `http_request` span and makes it the coroutine context
    // (may_tracing; never entered). Here the test plays the server: the middleware's
    // "Request started" / "Request completed" are events *on that span*, not spans of
    // their own (Epic 02.1).
    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let request_span = tracing::info_span!(parent: None, "http_request", path = "/pets/12345");
    let resp = may_tracing::with_span(request_span.clone(), || {
        dispatcher
            .dispatch(route_match, None, Default::default(), Default::default())
            .unwrap()
    });
    assert_eq!(resp.status, 200);
    drop(request_span);

    tracing.force_flush();
    tracing.wait_for_span("http_request");

    let spans = tracing.spans();
    let req = spans
        .iter()
        .find(|s| s.name == "http_request")
        .expect("http_request span exported");
    let events: Vec<&str> = req.events.iter().map(|e| e.name.as_ref()).collect();
    assert!(events.contains(&"Request started"), "events: {events:?}");
    assert!(events.contains(&"Request completed"), "events: {events:?}");
    assert!(
        !spans.iter().any(|s| s.name == "http_response"),
        "no throwaway http_response span any more"
    );
}
