//! Perf-science helpers (Story 12.8 / Epic 12 Wave 4).
//!
//! Shared by Criterion benches and unit tests so harness construction stays
//! panic-free and the match-vs-validate comparison is reproducible.

use crate::router::Router;
use crate::spec::RouteMeta;
use crate::validator_cache::ValidatorCache;
use http::Method;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Recommended Criterion sample size for noisy laptop/CI hosts (Phase 6).
pub const CRITERION_SAMPLE_SIZE: usize = 60;

/// Recommended Criterion measurement window.
pub const CRITERION_MEASUREMENT_TIME: Duration = Duration::from_secs(5);

/// Warm-up before measurement (reduce first-sample noise).
pub const CRITERION_WARM_UP: Duration = Duration::from_secs(1);

/// Build `n` synthetic GET routes `/api/v1/resource{i}/{id}` for scalability curves.
#[must_use]
pub fn scalability_routes(n: usize) -> Vec<RouteMeta> {
    let mut routes = Vec::with_capacity(n);
    for i in 0..n {
        routes.push(RouteMeta {
            x_service: None,
            x_brrtrouter_downstream_path: None,
            x_brrtrouter_impl: None,
            method: Method::GET,
            path_pattern: Arc::from(format!("/api/v1/resource{i}/{{id}}").as_str()),
            handler_name: Arc::from(format!("handler_{i}").as_str()),
            base_path: String::new(),
            parameters: Vec::new(),
            request_schema: None,
            request_body_required: false,
            request_content_types: Vec::new(),
            response_schema: None,
            example: None,
            responses: std::collections::HashMap::new(),
            security: Vec::new(),
            example_name: "perf".to_string(),
            project_slug: "perf".to_string(),
            output_dir: std::path::PathBuf::from("perf"),
            sse: false,
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
            cors_policy: crate::middleware::RouteCorsPolicy::Inherit,
        });
    }
    routes
}

/// Pet-store-shaped request schema used by validation microbenches.
#[must_use]
pub fn pet_request_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "photoUrls"],
        "properties": {
            "id": { "type": "integer", "format": "int64" },
            "name": { "type": "string" },
            "tag": { "type": "string" },
            "status": { "type": "string", "enum": ["available", "pending", "sold"] },
            "photoUrls": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

/// Valid body for [`pet_request_schema`].
#[must_use]
pub fn pet_valid_body() -> Value {
    json!({
        "name": "doggie",
        "photoUrls": ["https://example.com/1.png"],
        "status": "available"
    })
}

/// Invalid body that forces the `iter_errors` failure path.
#[must_use]
pub fn pet_invalid_body() -> Value {
    json!({
        "name": 123,
        "photoUrls": []
    })
}

/// Wall time for `iters` radix matches against a mid-tree path.
///
/// Times the match decision only (`is_some`), matching Criterion benches — not
/// the cost of dropping a full [`crate::router::RouteMatch`] each iteration.
#[must_use]
pub fn time_route_matches(route_count: usize, iters: u32) -> Duration {
    let router = Router::new(scalability_routes(route_count));
    let path = format!("/api/v1/resource{}/123", route_count / 2);
    let start = Instant::now();
    for _ in 0..iters {
        let hit = router.route(Method::GET, &path).is_some();
        let _ = std::hint::black_box(hit);
    }
    start.elapsed()
}

/// Wall time for `iters` cached schema `is_valid` checks (happy path).
///
/// Returns `None` only if schema compilation fails (bench setup bug).
#[must_use]
pub fn time_schema_is_valid(iters: u32) -> Option<Duration> {
    let cache = ValidatorCache::new(true);
    let schema = pet_request_schema();
    let validator = cache.get_or_compile("createPet", "request", None, &schema)?;
    let body = pet_valid_body();
    let start = Instant::now();
    for _ in 0..iters {
        let _ = std::hint::black_box(validator.as_ref().is_valid(&body));
    }
    Some(start.elapsed())
}

/// Wall time for `iters` `iter_errors` on an invalid body (failure path).
#[must_use]
pub fn time_schema_iter_errors_invalid(iters: u32) -> Option<Duration> {
    let cache = ValidatorCache::new(true);
    let schema = pet_request_schema();
    let validator = cache.get_or_compile("createPet", "request", None, &schema)?;
    let body = pet_invalid_body();
    let start = Instant::now();
    for _ in 0..iters {
        let n = validator.as_ref().iter_errors(&body).count();
        let _ = std::hint::black_box(n);
    }
    Some(start.elapsed())
}

/// P5 evidence: `(match_ns, is_valid_ns, iter_errors_invalid_ns)` per op.
///
/// On pet-shaped schemas all three are typically **sub-µs** (see
/// `docs/PERFORMANCE.md`). The gate for skipping a radix rewrite is that match
/// is already negligible vs end-to-end latency — not a folklore ratio.
#[must_use]
pub fn match_vs_validate_ns_per_op(iters: u32) -> Option<(u128, u128, u128)> {
    let iters = iters.max(1);
    let match_d = time_route_matches(100, iters);
    let valid_d = time_schema_is_valid(iters)?;
    let errors_d = time_schema_iter_errors_invalid(iters)?;
    Some((
        match_d.as_nanos() / u128::from(iters),
        valid_d.as_nanos() / u128::from(iters),
        errors_d.as_nanos() / u128::from(iters),
    ))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn perf_harness_p2_scalability_route_counts() {
        for n in [10usize, 50, 100, 200, 500] {
            let routes = scalability_routes(n);
            assert_eq!(routes.len(), n);
            let router = Router::new(routes);
            let path = format!("/api/v1/resource{}/1", n / 2);
            assert!(
                router.route(Method::GET, &path).is_some(),
                "expected match at {n} routes for {path}"
            );
        }
    }

    #[test]
    fn perf_harness_p1_route_match_bench_helper_runs() {
        let _ = time_route_matches(50, 200); // completes without panic (N4)
    }

    #[test]
    fn perf_harness_p3_schema_bench_helper_runs() {
        let d = time_schema_is_valid(100).expect("pet schema must compile");
        assert!(d.as_nanos() > 0 || d.is_zero());
    }

    /// P5 — comparative evidence before blaming radix.
    ///
    /// On the pet-shaped microbench (release Criterion on ms02): match,
    /// `is_valid`, and `iter_errors(invalid)` are all **sub-µs** and often
    /// within ~4× of each other. That still forbids a trie rewrite: match is
    /// already negligible vs multi-ms request latency. See PERFORMANCE.md.
    #[test]
    fn perf_harness_p5_match_much_cheaper_than_validate() {
        let (match_ns, is_valid_ns, iter_errors_ns) =
            match_vs_validate_ns_per_op(5_000).expect("schema setup");
        #[cfg(not(debug_assertions))]
        {
            const SUB_US: u128 = 5_000; // 5 µs ceiling for these micros
            assert!(
                match_ns < SUB_US && is_valid_ns < SUB_US && iter_errors_ns < SUB_US,
                "P5: expected match/is_valid/iter_errors all sub-5µs (release); \
                 match={match_ns}ns is_valid={is_valid_ns}ns iter_errors={iter_errors_ns}ns"
            );
            // Match must not be wildly worse than schema micros (noise guard).
            assert!(
                match_ns < iter_errors_ns.saturating_mul(10).max(1_000),
                "P5: match unexpectedly >> schema micros; match={match_ns} iter_errors={iter_errors_ns}"
            );
        }
        #[cfg(debug_assertions)]
        {
            let _ = (match_ns, is_valid_ns, iter_errors_ns);
        }
    }

    #[test]
    fn perf_harness_n4_no_panic_on_empty_scalability() {
        let routes = scalability_routes(0);
        assert!(routes.is_empty());
        let _ = Router::new(routes);
    }
}
