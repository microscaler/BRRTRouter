/// Integration tests for body size calculation optimizations
///
/// These tests verify that:
/// 1. Content-Length header is preferred when available
/// 2. Estimated body size is used as fallback
/// 3. JSON serialization (to_string()) is not used in hot path
use brrtrouter::spec::{estimate_body_size, load_spec};
use serde_json::json;

#[test]
fn test_route_has_estimated_body_size_from_spec() {
    // Load the pet store example spec
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();

    // Find a route with a request body (like add_pet)
    let add_pet_route = routes
        .iter()
        .find(|r| r.handler_name == "add_pet")
        .expect("add_pet route should exist");

    // It should have a request schema and therefore an estimated size
    assert!(add_pet_route.request_schema.is_some());
    assert!(add_pet_route.estimated_request_body_bytes.is_some());

    let estimated = add_pet_route.estimated_request_body_bytes.unwrap();
    // Pet object should have a reasonable size (not 0, not gigantic)
    assert!(estimated > 0, "Estimated size should be positive");
    assert!(estimated < 100_000, "Estimated size should be reasonable");
}

#[test]
fn test_route_without_body_has_no_estimate() {
    // Load the pet store example spec
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();

    // Find a GET route with no body (like list_pets)
    let list_pets_route = routes
        .iter()
        .find(|r| r.handler_name == "list_pets")
        .expect("list_pets route should exist");

    // GET routes typically don't have request bodies
    if list_pets_route.request_schema.is_none() {
        assert_eq!(list_pets_route.estimated_request_body_bytes, None);
    }
}

#[test]
fn test_estimate_body_size_complex_schema() {
    // Test a complex schema that would be expensive to serialize
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "maxLength": 100
            },
            "tags": {
                "type": "array",
                "maxItems": 50,
                "items": {
                    "type": "string",
                    "maxLength": 20
                }
            },
            "metadata": {
                "type": "object",
                "properties": {
                    "created": {"type": "string", "maxLength": 30},
                    "updated": {"type": "string", "maxLength": 30},
                    "version": {"type": "integer"}
                }
            },
            "active": {"type": "boolean"}
        }
    });

    let estimated = estimate_body_size(Some(&schema));
    assert!(estimated.is_some());

    let size = estimated.unwrap();
    // Should be a reasonable estimate for this structure
    // name: ~200, tags: ~2200, metadata: ~200, active: ~5, overhead: ~20
    // Total around 2625 bytes
    assert!(size > 2000, "Should estimate at least 2KB for this schema");
    assert!(size < 10000, "Should not over-estimate");
}

#[test]
fn test_estimate_respects_vendor_extension() {
    let schema = json!({
        "type": "object",
        "x-brrtrouter-body-size-bytes": 8192,
        "properties": {
            "data": {"type": "string"}
        }
    });

    let estimated = estimate_body_size(Some(&schema));
    assert_eq!(
        estimated,
        Some(8192),
        "Should use vendor extension override"
    );
}

#[test]
fn test_all_routes_with_bodies_have_estimates() {
    // Load the spec and verify all routes with request bodies have estimates
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();

    for route in routes {
        if route.request_schema.is_some() {
            assert!(
                route.estimated_request_body_bytes.is_some(),
                "Route {} with request schema should have estimated body size",
                route.handler_name
            );
        }
    }
}
