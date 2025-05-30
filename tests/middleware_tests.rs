use brrtrouter::{
    dispatcher::Dispatcher, load_spec, middleware::MetricsMiddleware, router::Router,
};
use http::Method;
use pet_store::registry;
use std::collections::HashMap;
use std::sync::Arc;

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;

#[test]
fn test_metrics_middleware_counts() {
    let _tracing = TestTracing::init();
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, HashMap::new(), HashMap::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(metrics.request_count(), 1);
    assert!(metrics.average_latency().as_nanos() > 0);
}

#[test]
fn test_metrics_stack_usage() {
    // set an odd stack size so may prints usage information
    may::config().set_stack_size(0x401);
    let mut tracing = TestTracing::init();

    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, HashMap::new(), HashMap::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    let (size, used) = metrics.stack_usage();
    assert_eq!(size, 0x401);
    assert!(used >= 0);
    tracing.wait_for_span("get_pet");
}
