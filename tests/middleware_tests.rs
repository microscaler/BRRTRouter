use brrtrouter::{
    dispatcher::Dispatcher,
    middleware::MetricsMiddleware,
    router::Router,
    load_spec,
};
use http::Method;
use pet_store::registry;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_metrics_middleware_counts() {
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());

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
    std::env::set_var("BRRTR_STACK_SIZE", "0x8001");
    may::config().set_stack_size(0x8001);

    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());

    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, HashMap::new(), HashMap::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    let (size, used) = metrics.stack_usage();
    assert_eq!(size, 0x8001);
    assert!(used >= 0);
}
