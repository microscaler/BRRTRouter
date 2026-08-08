//! Phase 6 comparative microbench (Story 12.8 / P5).
//!
//! Evidence for the next investment target:
//! - Happy-path `is_valid` ≈ route match (both tiny)
//! - Failure-path `iter_errors` ≫ match
//!
//! Do **not** justify a radix rewrite from folklore (N2 / N6).
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use brrtrouter::perf_harness::{
    pet_invalid_body, pet_request_schema, pet_valid_body, scalability_routes,
    CRITERION_MEASUREMENT_TIME, CRITERION_SAMPLE_SIZE, CRITERION_WARM_UP,
};
use brrtrouter::router::Router;
use brrtrouter::validator_cache::ValidatorCache;
use criterion::{criterion_group, criterion_main, Criterion};
use http::Method;
use std::hint::black_box;

fn phase6_criterion() -> Criterion {
    Criterion::default()
        .sample_size(CRITERION_SAMPLE_SIZE)
        .measurement_time(CRITERION_MEASUREMENT_TIME)
        .warm_up_time(CRITERION_WARM_UP)
}

fn bench_match_vs_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_vs_validate");

    let router = Router::new(scalability_routes(100));
    let path = "/api/v1/resource50/123";
    group.bench_function("route_match_100_routes", |b| {
        b.iter(|| {
            let res = router.route(Method::GET, black_box(path));
            black_box(res.is_some());
        })
    });

    let cache = ValidatorCache::new(true);
    let schema = pet_request_schema();
    let validator = cache
        .get_or_compile("createPet", "request", None, &schema)
        .expect("schema must compile");
    let body = pet_valid_body();
    let bad = pet_invalid_body();

    group.bench_function("schema_is_valid_pet", |b| {
        b.iter(|| {
            let ok = black_box(validator.as_ref()).is_valid(black_box(&body));
            black_box(ok);
        })
    });

    group.bench_function("schema_iter_errors_invalid_pet", |b| {
        b.iter(|| {
            let n = black_box(validator.as_ref())
                .iter_errors(black_box(&bad))
                .count();
            black_box(n);
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = phase6_criterion();
    targets = bench_match_vs_validate
}
criterion_main!(benches);
