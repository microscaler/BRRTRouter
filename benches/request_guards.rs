//! Optional Phase 6 Criterion groups for Stories 12.2 / 12.4 hot checks.
//!
//! These are microbenches of the **guard helpers** (not full HTTP). Use them to
//! catch regressions in body-limit / param-validation after Wave 0–1.
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::perf_harness::{
    CRITERION_MEASUREMENT_TIME, CRITERION_SAMPLE_SIZE, CRITERION_WARM_UP,
};
use brrtrouter::router::ParamVec;
use brrtrouter::server::body_limit::{
    body_exceeds_limit, content_length_for_limit, effective_inbound_body_limit,
};
use brrtrouter::server::param_validation::validate_route_parameters;
use brrtrouter::spec::{ParameterLocation, ParameterMeta, ParameterStyle};
use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;
use std::hint::black_box;
use std::sync::Arc;

fn phase6_criterion() -> Criterion {
    Criterion::default()
        .sample_size(CRITERION_SAMPLE_SIZE)
        .measurement_time(CRITERION_MEASUREMENT_TIME)
        .warm_up_time(CRITERION_WARM_UP)
}

fn bench_body_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("body_limit");

    group.bench_function("effective_inbound_body_limit", |b| {
        b.iter(|| {
            let lim = effective_inbound_body_limit(black_box(Some(1024 * 1024)));
            black_box(lim);
        })
    });

    group.bench_function("content_length_for_limit_ok", |b| {
        b.iter(|| {
            let r = content_length_for_limit(black_box(Some("4096")), black_box(16 * 1024 * 1024));
            black_box(r.is_ok());
        })
    });

    group.bench_function("body_exceeds_limit", |b| {
        b.iter(|| {
            let over = body_exceeds_limit(black_box(1025), black_box(1024));
            black_box(over);
        })
    });

    group.finish();
}

fn sample_params() -> (Vec<ParameterMeta>, ParamVec, ParamVec, HeaderVec, HeaderVec) {
    let parameters = vec![
        ParameterMeta {
            name: "id".into(),
            location: ParameterLocation::Path,
            required: true,
            schema: Some(json!({"type": "integer"})),
            style: Some(ParameterStyle::Simple),
            explode: None,
        },
        ParameterMeta {
            name: "limit".into(),
            location: ParameterLocation::Query,
            required: false,
            schema: Some(json!({"type": "integer"})),
            style: Some(ParameterStyle::Form),
            explode: None,
        },
        ParameterMeta {
            name: "X-Request-Id".into(),
            location: ParameterLocation::Header,
            required: true,
            schema: Some(json!({"type": "string"})),
            style: None,
            explode: None,
        },
    ];
    let mut path = ParamVec::new();
    path.push((Arc::from("id"), "42".into()));
    let mut query = ParamVec::new();
    query.push((Arc::from("limit"), "10".into()));
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("x-request-id"), "abc".into()));
    let cookies = HeaderVec::new();
    (parameters, path, query, headers, cookies)
}

fn bench_param_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("param_validation");
    let (parameters, path, query, headers, cookies) = sample_params();

    group.bench_function("validate_route_parameters_ok", |b| {
        b.iter(|| {
            let r = validate_route_parameters(
                black_box(&parameters),
                black_box(&path),
                black_box(&query),
                black_box(&headers),
                black_box(&cookies),
            );
            black_box(r.is_ok());
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = phase6_criterion();
    targets = bench_body_limit, bench_param_validation
}
criterion_main!(benches);
