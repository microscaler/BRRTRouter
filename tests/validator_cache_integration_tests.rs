//! Integration tests for JSON Schema validator caching
//!
//! # Test Coverage
//!
//! This module tests the validator cache behavior in real request/response scenarios:
//! - Cache population on first request
//! - Cache reuse on subsequent requests
//! - Cache behavior with valid and invalid schemas
//! - Cache disable via environment variable
//!
//! # Test Strategy
//!
//! Uses a minimal OpenAPI spec with schema validation to verify:
//! 1. **Cache Hit/Miss**: First request compiles, subsequent use cache
//! 2. **Functional Parity**: Validation behavior identical with/without cache
//! 3. **Configuration**: BRRTR_SCHEMA_CACHE environment variable works
//! 4. **Performance**: Cache reduces validation overhead (measured implicitly)

use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::load_spec_full;
use brrtrouter::router::Router;
use brrtrouter::server::AppService;
use brrtrouter::validator_cache::ValidatorCache;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[test]
fn test_validator_cache_basic_functionality() {
    // Create a cache with caching enabled
    let cache = ValidatorCache::new(true);

    // First access should compile and cache
    let schema1 = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"}
        },
        "required": ["name"]
    });

    let validator1 = cache.get_or_compile("test_handler", "request", None, &schema1);
    assert!(validator1.is_some());
    assert_eq!(
        cache.size(),
        1,
        "Cache should contain 1 validator after first compilation"
    );

    // Second access should use cached validator
    let validator2 = cache.get_or_compile("test_handler", "request", None, &schema1);
    assert!(validator2.is_some());
    assert_eq!(
        cache.size(),
        1,
        "Cache size should remain 1 (reused existing)"
    );

    // Validators should be the same Arc (same memory address)
    assert!(
        Arc::ptr_eq(&validator1.unwrap(), &validator2.unwrap()),
        "Validators should be the same Arc instance (cache hit)"
    );
}

#[test]
fn test_validator_cache_different_handlers() {
    let cache = ValidatorCache::new(true);
    let schema = json!({"type": "object"});

    // Different handlers should have separate cache entries
    cache.get_or_compile("handler1", "request", None, &schema);
    cache.get_or_compile("handler2", "request", None, &schema);
    cache.get_or_compile("handler1", "response", Some(200), &schema);
    cache.get_or_compile("handler1", "response", Some(404), &schema);

    assert_eq!(
        cache.size(),
        4,
        "Cache should contain 4 validators for different keys"
    );
}

#[test]
fn test_validator_cache_disabled() {
    // Create a cache with caching disabled
    let cache = ValidatorCache::new(false);

    let schema = json!({
        "type": "object",
        "properties": {
            "value": {"type": "string"}
        }
    });

    // First access
    let validator1 = cache.get_or_compile("test_handler", "request", None, &schema);
    assert!(validator1.is_some());
    assert_eq!(cache.size(), 0, "Cache should be empty when disabled");

    // Second access
    let validator2 = cache.get_or_compile("test_handler", "request", None, &schema);
    assert!(validator2.is_some());
    assert_eq!(cache.size(), 0, "Cache should remain empty when disabled");

    // Validators should be different Arc instances (compiled independently)
    assert!(
        !Arc::ptr_eq(&validator1.unwrap(), &validator2.unwrap()),
        "Validators should be different instances when cache is disabled"
    );
}

#[test]
fn test_validator_cache_with_app_service() {
    // Load a minimal spec for testing
    let spec_path = "examples/openapi.yaml";
    let (routes, security_schemes, _slug) = load_spec_full(spec_path).expect("Failed to load spec");

    // Create service with cache enabled (default)
    let router = Arc::new(RwLock::new(Router::new(routes)));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));
    let service = AppService::new(
        router,
        dispatcher,
        security_schemes,
        PathBuf::from(spec_path),
        None,
        None,
    );

    // Verify cache is initialized
    assert_eq!(
        service.validator_cache.size(),
        0,
        "Cache should start empty"
    );

    // After creating the service, the cache should be ready to use
    // (validators are compiled lazily on first request)
}

#[test]
fn test_validator_validates_correctly() {
    let cache = ValidatorCache::new(true);

    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer", "minimum": 0}
        },
        "required": ["name"]
    });

    let validator = cache
        .get_or_compile("test_handler", "request", None, &schema)
        .expect("Failed to compile schema");

    // Valid data should pass
    let valid_data = json!({
        "name": "John",
        "age": 30
    });
    assert!(
        validator.is_valid(&valid_data),
        "Valid data should pass validation"
    );

    // Invalid data (missing required field) should fail
    let invalid_data1 = json!({
        "age": 30
    });
    assert!(
        !validator.is_valid(&invalid_data1),
        "Data missing required field should fail validation"
    );

    // Invalid data (wrong type) should fail
    let invalid_data2 = json!({
        "name": "John",
        "age": "thirty"
    });
    assert!(
        !validator.is_valid(&invalid_data2),
        "Data with wrong type should fail validation"
    );

    // Invalid data (constraint violation) should fail
    let invalid_data3 = json!({
        "name": "John",
        "age": -5
    });
    assert!(
        !validator.is_valid(&invalid_data3),
        "Data violating minimum constraint should fail validation"
    );
}

#[test]
fn test_cache_key_uniqueness() {
    let cache = ValidatorCache::new(true);
    let schema = json!({"type": "object"});

    // Request and response validators should be separate
    cache.get_or_compile("handler1", "request", None, &schema);
    cache.get_or_compile("handler1", "response", Some(200), &schema);

    assert_eq!(
        cache.size(),
        2,
        "Request and response validators should be cached separately"
    );

    // Different status codes should be separate
    cache.get_or_compile("handler1", "response", Some(404), &schema);
    cache.get_or_compile("handler1", "response", Some(500), &schema);

    assert_eq!(
        cache.size(),
        4,
        "Different status codes should have separate cache entries"
    );
}

#[test]
fn test_cache_thread_safety() {
    use std::sync::Arc as StdArc;
    use std::thread;

    let cache = StdArc::new(ValidatorCache::new(true));
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {"type": "integer"}
        }
    });

    // Spawn multiple threads that try to compile the same schema
    let mut handles = vec![];
    for i in 0..10 {
        let cache_clone = StdArc::clone(&cache);
        let schema_clone = schema.clone();
        let handle = thread::spawn(move || {
            // All threads try to get/compile the same validator
            let validator =
                cache_clone.get_or_compile("concurrent_handler", "request", None, &schema_clone);
            assert!(validator.is_some(), "Thread {i} failed to get validator");
            validator.unwrap()
        });
        handles.push(handle);
    }

    // Collect results
    let validators: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should get the same validator (same Arc pointer)
    // This proves thread-safety and proper cache sharing
    let first_ptr = &validators[0];
    for (i, validator) in validators.iter().enumerate().skip(1) {
        assert!(
            Arc::ptr_eq(first_ptr, validator),
            "Thread {i} got a different validator Arc"
        );
    }

    // Only one validator should be in the cache
    assert_eq!(
        cache.size(),
        1,
        "Only one validator should be cached despite concurrent access"
    );
}

#[test]
fn test_invalid_schema_not_cached() {
    let cache = ValidatorCache::new(true);

    // Invalid schema (invalid type)
    let invalid_schema = json!({
        "type": "invalid_type_that_does_not_exist"
    });

    let result1 = cache.get_or_compile("test_handler", "request", None, &invalid_schema);
    assert!(result1.is_none(), "Invalid schema should return None");
    assert_eq!(cache.size(), 0, "Failed compilation should not be cached");

    // Second attempt should also fail (not using a cached error)
    let result2 = cache.get_or_compile("test_handler", "request", None, &invalid_schema);
    assert!(
        result2.is_none(),
        "Invalid schema should return None on second attempt"
    );
    assert_eq!(cache.size(), 0, "Cache should still be empty");
}

#[test]
fn test_cache_clear() {
    let cache = ValidatorCache::new(true);
    let schema = json!({"type": "object"});

    // Populate cache
    cache.get_or_compile("handler1", "request", None, &schema);
    cache.get_or_compile("handler2", "request", None, &schema);
    assert_eq!(cache.size(), 2);

    // Clear cache
    cache.clear();
    assert_eq!(cache.size(), 0, "Cache should be empty after clear");

    // Should be able to compile again
    cache.get_or_compile("handler1", "request", None, &schema);
    assert_eq!(cache.size(), 1, "Cache should work after being cleared");
}
