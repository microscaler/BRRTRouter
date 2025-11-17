//! Stack size computation for coroutine handlers
//!
//! This module provides heuristic-based stack size computation for per-handler
//! coroutine stacks. The stack size is determined by analyzing OpenAPI operations
//! and their associated schemas.

use crate::spec::{ParameterLocation, RouteMeta};
use serde_json::Value;
use std::collections::HashSet;

/// Default base stack size in bytes (16 KiB)
pub const BASE_STACK_SIZE: usize = 16 * 1024;

/// Minimum allowed stack size in bytes (16 KiB)
pub const MIN_STACK_SIZE: usize = 16 * 1024;

/// Maximum allowed stack size in bytes (256 KiB)
pub const MAX_STACK_SIZE: usize = 256 * 1024;

/// Additional stack size per 5 path/query/header parameters (4 KiB)
const STACK_PER_5_PARAMS: usize = 4 * 1024;

/// Additional stack size for SSE/streaming endpoints (8 KiB)
const STACK_SSE_BONUS: usize = 8 * 1024;

/// Additional stack size for moderately deep schemas (>6 depth, 4 KiB)
const STACK_MODERATE_DEPTH: usize = 4 * 1024;

/// Additional stack size for very deep schemas (>12 depth, 16 KiB total)
const STACK_VERY_DEEP: usize = 16 * 1024;

/// Compute recommended stack size for a handler based on OpenAPI signals
///
/// The computation uses the following heuristics:
/// - Base stack size: 16 KiB
/// - Add 4 KiB for every 5 path/query/header parameters
/// - Add 4-16 KiB for deep schemas based on depth tiers (>6, >12)
/// - Add 8 KiB for SSE/streaming endpoints
/// - Clamp to [16 KiB, 256 KiB] with environment tunables
///
/// # Arguments
///
/// * `route` - Route metadata containing parameters and schemas
///
/// # Returns
///
/// Computed stack size in bytes, clamped to the allowed range
pub fn compute_stack_size(route: &RouteMeta) -> usize {
    // Check for vendor extension override first
    if let Some(vendor_stack_size) = route.x_brrtrouter_stack_size {
        return clamp_stack_size(vendor_stack_size);
    }

    let mut stack_size = BASE_STACK_SIZE;

    // Count path/query/header parameters
    let relevant_param_count = route
        .parameters
        .iter()
        .filter(|p| matches!(
            p.location,
            ParameterLocation::Path | ParameterLocation::Query | ParameterLocation::Header
        ))
        .count();

    // Add 4 KiB for every 5 parameters
    if relevant_param_count > 0 {
        let param_chunks = (relevant_param_count + 4) / 5; // ceiling division
        stack_size += param_chunks * STACK_PER_5_PARAMS;
    }

    // Analyze schema depth
    let max_depth = compute_max_schema_depth(route);
    if max_depth > 12 {
        stack_size += STACK_VERY_DEEP;
    } else if max_depth > 6 {
        stack_size += STACK_MODERATE_DEPTH;
    }

    // Add bonus for SSE/streaming endpoints
    if route.sse {
        stack_size += STACK_SSE_BONUS;
    }

    // Clamp to allowed range
    clamp_stack_size(stack_size)
}

/// Compute the maximum depth of schemas in a route
///
/// Analyzes both request and response schemas to find the deepest nesting level.
///
/// # Arguments
///
/// * `route` - Route metadata containing schemas
///
/// # Returns
///
/// Maximum depth found in any schema (0 if no schemas)
fn compute_max_schema_depth(route: &RouteMeta) -> usize {
    let mut max_depth = 0;

    // Check request schema
    if let Some(ref schema) = route.request_schema {
        let depth = schema_depth(schema, &mut HashSet::new());
        max_depth = max_depth.max(depth);
    }

    // Check response schema
    if let Some(ref schema) = route.response_schema {
        let depth = schema_depth(schema, &mut HashSet::new());
        max_depth = max_depth.max(depth);
    }

    // Check all response variants
    for response_map in route.responses.values() {
        for response_spec in response_map.values() {
            if let Some(ref schema) = response_spec.schema {
                let depth = schema_depth(schema, &mut HashSet::new());
                max_depth = max_depth.max(depth);
            }
        }
    }

    max_depth
}

/// Recursively compute the depth of a JSON Schema
///
/// Tracks visited references to prevent infinite recursion.
///
/// # Arguments
///
/// * `schema` - JSON Schema value to analyze
/// * `visited` - Set of visited schema references (for cycle detection)
///
/// # Returns
///
/// Maximum nesting depth of the schema
fn schema_depth(schema: &Value, visited: &mut HashSet<String>) -> usize {
    match schema {
        Value::Object(obj) => {
            // Handle $ref
            if let Some(Value::String(ref_str)) = obj.get("$ref") {
                // Check if we've already visited this reference to prevent infinite recursion
                if visited.contains(ref_str) {
                    return 0;
                }
                visited.insert(ref_str.clone());
                // For now, we can't resolve refs without access to the full spec
                // Assume a moderate depth for referenced schemas
                return 3;
            }

            let mut max_child_depth = 0;

            // Check properties (object type)
            if let Some(Value::Object(props)) = obj.get("properties") {
                for prop_schema in props.values() {
                    let depth = schema_depth(prop_schema, visited);
                    max_child_depth = max_child_depth.max(depth);
                }
                return 1 + max_child_depth;
            }

            // Check items (array type)
            if let Some(items) = obj.get("items") {
                let depth = schema_depth(items, visited);
                return 1 + depth;
            }

            // Check allOf, anyOf, oneOf
            for key in &["allOf", "anyOf", "oneOf"] {
                if let Some(Value::Array(schemas)) = obj.get(*key) {
                    for sub_schema in schemas {
                        let depth = schema_depth(sub_schema, visited);
                        max_child_depth = max_child_depth.max(depth);
                    }
                    return 1 + max_child_depth;
                }
            }

            // Check additionalProperties
            if let Some(additional) = obj.get("additionalProperties") {
                if additional.is_object() {
                    let depth = schema_depth(additional, visited);
                    return 1 + depth;
                }
            }

            // No nested structure found
            0
        }
        Value::Bool(_) => {
            // Boolean schemas (true/false) have no depth
            0
        }
        _ => {
            // Primitive or invalid schema
            0
        }
    }
}

/// Clamp stack size to the allowed range
///
/// Ensures the stack size is between MIN_STACK_SIZE and MAX_STACK_SIZE.
/// Can be overridden with environment variables:
/// - BRRTR_STACK_MIN_BYTES: Minimum stack size (default 16 KiB)
/// - BRRTR_STACK_MAX_BYTES: Maximum stack size (default 256 KiB)
///
/// # Arguments
///
/// * `size` - Computed stack size in bytes
///
/// # Returns
///
/// Clamped stack size in bytes
fn clamp_stack_size(size: usize) -> usize {
    let min = std::env::var("BRRTR_STACK_MIN_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(MIN_STACK_SIZE);

    let max = std::env::var("BRRTR_STACK_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(MAX_STACK_SIZE);

    size.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ParameterLocation, ParameterMeta, RouteMeta};
    use http::Method;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_route() -> RouteMeta {
        RouteMeta {
            method: Method::GET,
            path_pattern: "/test".to_string(),
            handler_name: "test_handler".to_string(),
            parameters: vec![],
            request_schema: None,
            request_body_required: false,
            response_schema: None,
            example: None,
            responses: HashMap::new(),
            security: vec![],
            example_name: "".to_string(),
            project_slug: "test".to_string(),
            output_dir: PathBuf::from("/tmp"),
            base_path: "/".to_string(),
            sse: false,
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
        }
    }

    #[test]
    fn test_base_stack_size() {
        let route = create_test_route();
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE);
    }

    #[test]
    fn test_stack_size_with_parameters() {
        let mut route = create_test_route();
        // Add 5 path parameters (should add 4 KiB)
        for i in 0..5 {
            route.parameters.push(ParameterMeta {
                name: format!("param{}", i),
                location: ParameterLocation::Path,
                required: true,
                schema: None,
                style: None,
                explode: None,
            });
        }
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE + STACK_PER_5_PARAMS);
    }

    #[test]
    fn test_stack_size_with_10_parameters() {
        let mut route = create_test_route();
        // Add 10 parameters (should add 8 KiB)
        for i in 0..10 {
            route.parameters.push(ParameterMeta {
                name: format!("param{}", i),
                location: ParameterLocation::Path,
                required: true,
                schema: None,
                style: None,
                explode: None,
            });
        }
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE + 2 * STACK_PER_5_PARAMS);
    }

    #[test]
    fn test_stack_size_ignores_cookie_parameters() {
        let mut route = create_test_route();
        // Add 5 cookie parameters (should not affect stack size)
        for i in 0..5 {
            route.parameters.push(ParameterMeta {
                name: format!("param{}", i),
                location: ParameterLocation::Cookie,
                required: true,
                schema: None,
                style: None,
                explode: None,
            });
        }
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE);
    }

    #[test]
    fn test_stack_size_with_sse() {
        let mut route = create_test_route();
        route.sse = true;
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE + STACK_SSE_BONUS);
    }

    #[test]
    fn test_stack_size_with_deep_schema() {
        let mut route = create_test_route();
        // Create a deeply nested schema (depth > 6)
        route.request_schema = Some(json!({
            "type": "object",
            "properties": {
                "level1": {
                    "type": "object",
                    "properties": {
                        "level2": {
                            "type": "object",
                            "properties": {
                                "level3": {
                                    "type": "object",
                                    "properties": {
                                        "level4": {
                                            "type": "object",
                                            "properties": {
                                                "level5": {
                                                    "type": "object",
                                                    "properties": {
                                                        "level6": {
                                                            "type": "object",
                                                            "properties": {
                                                                "level7": {
                                                                    "type": "string"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }));
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE + STACK_MODERATE_DEPTH);
    }

    #[test]
    fn test_stack_size_with_very_deep_schema() {
        let mut route = create_test_route();
        // Create a very deeply nested schema (depth > 12)
        let mut schema = json!({ "type": "string" });
        for _ in 0..13 {
            schema = json!({
                "type": "object",
                "properties": {
                    "nested": schema
                }
            });
        }
        route.response_schema = Some(schema);
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, BASE_STACK_SIZE + STACK_VERY_DEEP);
    }

    #[test]
    fn test_schema_depth_simple() {
        let schema = json!({ "type": "string" });
        let depth = schema_depth(&schema, &mut HashSet::new());
        assert_eq!(depth, 0);
    }

    #[test]
    fn test_schema_depth_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let depth = schema_depth(&schema, &mut HashSet::new());
        assert_eq!(depth, 1);
    }

    #[test]
    fn test_schema_depth_nested_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        });
        let depth = schema_depth(&schema, &mut HashSet::new());
        assert_eq!(depth, 2);
    }

    #[test]
    fn test_schema_depth_array() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }
        });
        let depth = schema_depth(&schema, &mut HashSet::new());
        assert_eq!(depth, 2);
    }

    #[test]
    fn test_clamp_stack_size_within_range() {
        let size = 32 * 1024;
        assert_eq!(clamp_stack_size(size), size);
    }

    #[test]
    fn test_clamp_stack_size_below_min() {
        let size = 8 * 1024;
        assert_eq!(clamp_stack_size(size), MIN_STACK_SIZE);
    }

    #[test]
    fn test_clamp_stack_size_above_max() {
        let size = 512 * 1024;
        assert_eq!(clamp_stack_size(size), MAX_STACK_SIZE);
    }

    #[test]
    fn test_combined_heuristics() {
        let mut route = create_test_route();
        
        // Add 7 parameters (ceiling(7/5) = 2 chunks = 8 KiB)
        for i in 0..7 {
            route.parameters.push(ParameterMeta {
                name: format!("param{}", i),
                location: ParameterLocation::Query,
                required: true,
                schema: None,
                style: None,
                explode: None,
            });
        }
        
        // Add deep schema (>6 depth = 4 KiB)
        route.request_schema = Some(json!({
            "type": "object",
            "properties": {
                "l1": { "type": "object", "properties": {
                    "l2": { "type": "object", "properties": {
                        "l3": { "type": "object", "properties": {
                            "l4": { "type": "object", "properties": {
                                "l5": { "type": "object", "properties": {
                                    "l6": { "type": "object", "properties": {
                                        "l7": { "type": "string" }
                                    }}
                                }}
                            }}
                        }}
                    }}
                }}
            }
        }));
        
        // Enable SSE (8 KiB)
        route.sse = true;
        
        let stack_size = compute_stack_size(&route);
        // 16 KiB base + 8 KiB params + 4 KiB depth + 8 KiB SSE = 36 KiB
        assert_eq!(stack_size, BASE_STACK_SIZE + 2 * STACK_PER_5_PARAMS + STACK_MODERATE_DEPTH + STACK_SSE_BONUS);
    }

    #[test]
    fn test_environment_variable_clamping() {
        // Test that env vars work for clamping
        std::env::set_var("BRRTR_STACK_MIN_BYTES", "32768"); // 32 KiB
        std::env::set_var("BRRTR_STACK_MAX_BYTES", "65536"); // 64 KiB
        
        // Test clamping to min
        let size_below_min = 16 * 1024;
        assert_eq!(clamp_stack_size(size_below_min), 32 * 1024);
        
        // Test clamping to max
        let size_above_max = 128 * 1024;
        assert_eq!(clamp_stack_size(size_above_max), 64 * 1024);
        
        // Test within range
        let size_in_range = 48 * 1024;
        assert_eq!(clamp_stack_size(size_in_range), 48 * 1024);
        
        // Clean up
        std::env::remove_var("BRRTR_STACK_MIN_BYTES");
        std::env::remove_var("BRRTR_STACK_MAX_BYTES");
    }

    #[test]
    fn test_vendor_extension_override() {
        let mut route = create_test_route();
        // Set vendor extension to 32 KiB
        route.x_brrtrouter_stack_size = Some(32 * 1024);
        
        // Add parameters and SSE (which would normally add to stack size)
        for i in 0..10 {
            route.parameters.push(ParameterMeta {
                name: format!("param{}", i),
                location: ParameterLocation::Path,
                required: true,
                schema: None,
                style: None,
                explode: None,
            });
        }
        route.sse = true;
        
        // Vendor extension should take precedence
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, 32 * 1024);
    }

    #[test]
    fn test_vendor_extension_clamped() {
        let mut route = create_test_route();
        // Set vendor extension to 512 KiB (above max)
        route.x_brrtrouter_stack_size = Some(512 * 1024);
        
        // Should be clamped to MAX_STACK_SIZE (256 KiB)
        let stack_size = compute_stack_size(&route);
        assert_eq!(stack_size, MAX_STACK_SIZE);
    }
}
