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

    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, Default::default(), Default::default())
        .unwrap();
    assert_eq!(resp.status, 200);

    // Wait for spans
    tracing.wait_for_span("get_pet");
    let spans = tracing.spans();
    assert!(spans.iter().any(|s| s.name.contains("get_pet")));
}
