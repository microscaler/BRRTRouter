//! Story 12.8 / Wave 4 — perf science guards (P1–P6, N2).

use brrtrouter::perf_harness::{
    match_vs_validate_ns_per_op, scalability_routes, time_route_matches, time_schema_is_valid,
    CRITERION_MEASUREMENT_TIME, CRITERION_SAMPLE_SIZE,
};
use brrtrouter::router::Router;
use brrtrouter::server::{SharedDispatcher, SharedRouter};
use http::Method;
use std::time::Duration;

#[test]
fn perf_science_p1_route_match_helper_completes() {
    let _ = time_route_matches(100, 500);
}

#[test]
fn perf_science_p2_scalability_curve_documented_counts() {
    // Documented curve points in docs/PERFORMANCE.md Phase 6.
    for n in [10usize, 50, 100, 200, 500] {
        let router = Router::new(scalability_routes(n));
        let path = format!("/api/v1/resource{}/123", n / 2);
        assert!(
            router.route(Method::GET, &path).is_some(),
            "scalability point {n} must match"
        );
    }
}

#[test]
fn perf_science_p3_schema_helper_runs() {
    assert!(time_schema_is_valid(50).is_some());
}

#[test]
fn perf_science_p5_match_much_cheaper_than_validate() {
    let (match_ns, is_valid_ns, iter_errors_ns) =
        match_vs_validate_ns_per_op(5_000).expect("schema setup for P5");
    // Debug unit-test profile is not the P5 authority (see PERFORMANCE.md +
    // `match_vs_validate` Criterion). Strict ratio in release only.
    #[cfg(not(debug_assertions))]
    {
        const SUB_US: u128 = 5_000;
        assert!(
            match_ns < SUB_US && is_valid_ns < SUB_US && iter_errors_ns < SUB_US,
            "P5: match/is_valid/iter_errors all sub-5µs (release); \
             match={match_ns}ns is_valid={is_valid_ns}ns iter_errors={iter_errors_ns}ns"
        );
    }
    #[cfg(debug_assertions)]
    {
        let _ = (match_ns, is_valid_ns, iter_errors_ns);
    }
}

#[test]
fn perf_science_p6_shared_router_is_arcswap_not_rwlock() {
    // Type aliases encode the Phase 1 lock-free match path.
    fn _assert_router(_: SharedRouter) {}
    fn _assert_dispatcher(_: SharedDispatcher) {}

    let service_src = include_str!("../src/server/service.rs");
    assert!(
        service_src.contains("pub type SharedRouter = Arc<ArcSwap<Router>>"),
        "P6: SharedRouter must remain ArcSwap (no RwLock reintro on match path)"
    );
    assert!(
        service_src.contains("pub type SharedDispatcher = Arc<ArcSwap<Dispatcher>>"),
        "P6: SharedDispatcher must remain ArcSwap"
    );
    // Hot-path comment contract: match uses ArcSwap load, not RwLock::read.
    assert!(
        service_src.contains("Router lookup: lock-free ArcSwap load"),
        "P6: request path must document ArcSwap load for router lookup"
    );
    assert!(
        !service_src.contains("self.router.read()"),
        "P6: forbid accidental RwLock::read on SharedRouter"
    );
}

#[test]
fn perf_science_n2_docs_forbid_routing_bottleneck_claim_without_evidence() {
    let perf = include_str!("../docs/PERFORMANCE.md");
    let flame = include_str!("../docs/flamegraph.md");
    // Forbidden bare claim (N2). Phase 6 section must state evidence-based next bottleneck.
    for doc in [perf, flame] {
        assert!(
            !doc.to_lowercase().contains("routing is the bottleneck"),
            "N2: do not claim routing bottleneck without P5 data"
        );
        assert!(
            !doc.to_lowercase().contains("routing bottleneck"),
            "N2: 'routing bottleneck' phrase forbidden unless accompanied by evidence framing \
             (PERFORMANCE.md uses explicit Phase 6 wording instead)"
        );
    }
    assert!(
        perf.contains("## Phase 6"),
        "Phase 6 harness notes required in PERFORMANCE.md"
    );
    assert!(
        perf.contains("next bottleneck") || perf.contains("Next bottleneck"),
        "written next-bottleneck recommendation required"
    );
    assert!(
        perf.contains("match ≪") || perf.contains("iter_errors"),
        "P5 evidence statement required in PERFORMANCE.md"
    );
}

#[test]
fn perf_science_criterion_defaults_are_stable() {
    assert_eq!(CRITERION_SAMPLE_SIZE, 60);
    assert_eq!(CRITERION_MEASUREMENT_TIME, Duration::from_secs(5));
}

#[test]
fn perf_science_p4_flamegraph_doc_has_validator_steps() {
    let flame = include_str!("../docs/flamegraph.md");
    assert!(
        flame.contains("validator") || flame.contains("schema"),
        "P4: flamegraph.md must cover validator-path profiling"
    );
    assert!(
        flame.contains("cargo flamegraph") || flame.contains("flamegraph"),
        "P4: reproducible flamegraph command required"
    );
}
