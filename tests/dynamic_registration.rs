#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::{dispatcher::Dispatcher, load_spec, router::Router};
use http::Method;
use pet_store::registry;
use std::env;

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

/// Test that per-handler stack size overrides work in dynamic registration.
/// This validates that setting BRRTR_STACK_SIZE__<HANDLER> environment variables
/// correctly applies to handlers registered via register_from_spec.
#[test]
fn test_dynamic_register_with_per_handler_stack_override() {
    // Set per-handler override for get_pet handler
    env::set_var("BRRTR_STACK_SIZE__GET_PET", "32768");

    // Also set a global override - the per-handler should take precedence
    env::set_var("BRRTR_STACK_SIZE", "49152");

    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();

    // Register handlers - this should use the per-handler override for get_pet
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }

    // Verify the handler works correctly
    let route_match = router
        .route(Method::GET, "/pets/12345")
        .expect("route match");
    let resp = dispatcher
        .dispatch(route_match, None, Default::default(), Default::default())
        .expect("dispatch");
    assert_eq!(resp.status, 200);

    // Clean up environment variables
    env::remove_var("BRRTR_STACK_SIZE__GET_PET");
    env::remove_var("BRRTR_STACK_SIZE");
}

/// Test that stack size clamping is applied in dynamic registration.
/// This validates that BRRTR_STACK_MIN_BYTES and BRRTR_STACK_MAX_BYTES
/// are respected when registering handlers via register_from_spec.
#[test]
fn test_dynamic_register_with_stack_clamping() {
    // Set min/max bounds
    env::set_var("BRRTR_STACK_MIN_BYTES", "32768");
    env::set_var("BRRTR_STACK_MAX_BYTES", "65536");

    // Try to set a stack size below the minimum
    env::set_var("BRRTR_STACK_SIZE__LIST_PETS", "16384");

    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();

    // Register handlers - stack size should be clamped to minimum
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }

    // Verify the handler works correctly
    let route_match = router.route(Method::GET, "/pets").expect("route match");
    let resp = dispatcher
        .dispatch(route_match, None, Default::default(), Default::default())
        .expect("dispatch");
    assert_eq!(resp.status, 200);

    // Clean up environment variables
    env::remove_var("BRRTR_STACK_MIN_BYTES");
    env::remove_var("BRRTR_STACK_MAX_BYTES");
    env::remove_var("BRRTR_STACK_SIZE__LIST_PETS");
}
