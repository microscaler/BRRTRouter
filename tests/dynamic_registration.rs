use brrtrouter::{dispatcher::Dispatcher, load_spec, router::Router};
use http::Method;
use pet_store::registry;

#[test]
fn test_dynamic_register_get_pet() {
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }

    let route_match = router
        .route(Method::GET, "/pets/12345")
        .expect("route match");
    let resp = dispatcher
        .dispatch(route_match, None, Default::default(), Default::default())
        .expect("dispatch");
    assert_eq!(resp.status, 200);
}

#[test]
fn test_dynamic_register_post_item() {
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }

    let route_match = router
        .route(Method::POST, "/items/item-001")
        .expect("route match");
    let resp = dispatcher
        .dispatch(
            route_match,
            Some(serde_json::json!({"name": "New Item"})),
            Default::default(),
            Default::default(),
        )
        .expect("dispatch");
    assert_eq!(resp.status, 200);
}
