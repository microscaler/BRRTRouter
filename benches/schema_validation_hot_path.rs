//! Criterion microbench for the JSON Schema **runtime** validation path used in
//! `AppService::call` (`ValidatorCache::get_or_compile` + `Validator::iter_errors`).
//!
//! Complements the macro **2000 users × 600 s** stress test: use this for sub-~15 %
//! comparisons without thermal drift (see `docs/llmwiki/topics/bench-harness-phase-6.md`).
//!
//! Criterion setup is infallible test data; the workspace still warns on `expect`/`unwrap`
//! in application code — opt out here only.
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use brrtrouter::validator_cache::ValidatorCache;
use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;
use std::hint::black_box;

fn pet_request_schema() -> serde_json::Value {
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

fn valid_body() -> serde_json::Value {
    json!({
        "name": "doggie",
        "photoUrls": ["https://example.com/1.png"],
        "status": "available"
    })
}

fn bench_iter_errors(c: &mut Criterion) {
    let cache = ValidatorCache::new(true);
    let schema = pet_request_schema();
    let validator = cache
        .get_or_compile("createPet", "request", None, &schema)
        .expect("schema must compile for bench");

    let ok = valid_body();
    c.bench_function("schema_iter_errors_valid_body", |b| {
        b.iter(|| {
            let n = black_box(validator.as_ref())
                .iter_errors(black_box(&ok))
                .count();
            black_box(n);
        })
    });

    let bad = json!({"name": 123, "photoUrls": []});
    c.bench_function("schema_iter_errors_invalid_body", |b| {
        b.iter(|| {
            let n = black_box(validator.as_ref())
                .iter_errors(black_box(&bad))
                .count();
            black_box(n);
        })
    });
}

fn bench_cache_hit(c: &mut Criterion) {
    let cache = ValidatorCache::new(true);
    let schema = pet_request_schema();
    let _warm = cache
        .get_or_compile("createPet", "request", None, &schema)
        .expect("schema must compile");

    c.bench_function("schema_cache_get_or_compile_hit", |b| {
        b.iter(|| {
            let v = cache.get_or_compile("createPet", "request", None, black_box(&schema));
            black_box(v.is_some());
        })
    });
}

criterion_group!(schema_benches, bench_iter_errors, bench_cache_hit);
criterion_main!(schema_benches);
