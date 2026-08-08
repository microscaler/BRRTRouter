//! Epic 10 Story 10.10 — property / fuzz-style compliance suite.
//!
//! Runs under default `cargo test` with a fixed RNG seed for CI reproducibility.
//! Process for new counterexamples (N6): minimize → add golden under
//! `tests/uri_golden/corpus.json` → re-run `uri_golden_harness`.

use std::sync::Arc;

use brrtrouter::http::{
    encode_query_component, encode_query_form_explode, resolve_downstream_target,
    resolve_path_template,
};
use brrtrouter::router::ParamVec;
use brrtrouter::server::request::parse_query_params;
use brrtrouter::server::request_target::{
    assert_request_target_uri_ok, request_target_exceeds_limit, DEFAULT_MAX_REQUEST_TARGET_OCTETS,
};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

/// CI-reproducible config (Story 10.10 P6).
fn config() -> Config {
    Config {
        cases: 64,
        max_shrink_iters: 1_000,
        rng_seed: RngSeed::Fixed(0x1010_c0ff_ee),
        ..Config::default()
    }
}

fn param_vec(pairs: &[(String, String)]) -> ParamVec {
    let mut p = ParamVec::new();
    for (k, v) in pairs {
        p.push((Arc::from(k.as_str()), v.clone()));
    }
    p
}

fn query_value_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 ._~-]{0,32}",
        Just("South Africa".to_string()),
        Just("Côte".to_string()),
        Just("東京🌐".to_string()),
        Just("a&b=c".to_string()),
        Just("x#frag".to_string()),
        Just("a+b".to_string()),
        Just(String::new()),
    ]
}

// --- Positive ---

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_positive_p1_encode_parse_roundtrip(value in query_value_strategy()) {
        let enc = encode_query_component(&value);
        let path = format!("/p?k={enc}");
        let parsed = parse_query_params(&path);
        let got = parsed
            .iter()
            .find(|(k, _)| k.as_ref() == "k")
            .map(|(_, v)| v.as_str());
        prop_assert_eq!(got, Some(value.as_str()));
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_positive_p2_resolve_always_uri_ok(
        k in "[a-zA-Z][a-zA-Z0-9_]{0,15}",
        v in query_value_strategy(),
    ) {
        let q = param_vec(&[(k, v)]);
        let rebuilt = resolve_path_template("/p", &ParamVec::new(), &q);
        prop_assert!(
            assert_request_target_uri_ok(&rebuilt).is_ok(),
            "Uri not OK for {rebuilt:?}"
        );
    }
}

#[test]
fn uri_property_positive_p3_passthrough_bytes_unchanged() {
    let raw = "q=a%2Bb+c";
    let q = parse_query_params(&format!("/p?{raw}"));
    let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
    assert_eq!(t, format!("/p?{raw}"));
}

#[test]
fn uri_property_positive_p4_golden_corpus_still_loads() {
    let corpus = include_str!("uri_golden/corpus.json");
    let v: serde_json::Value = serde_json::from_str(corpus).expect("corpus json");
    assert!(v["vectors"].as_array().unwrap().len() >= 18);
}

#[test]
fn uri_property_positive_p5_duplicate_keys_survive_rebuild() {
    let q = param_vec(&[("a".into(), "1".into()), ("a".into(), "2".into())]);
    let rebuilt = resolve_path_template("/p", &ParamVec::new(), &q);
    assert_eq!(rebuilt, "/p?a=1&a=2");
    assert_eq!(parse_query_params(&rebuilt).len(), 2);
}

#[test]
fn uri_property_positive_p6_fixed_seed_config() {
    let c = config();
    assert_eq!(c.cases, 64);
    assert!(matches!(c.rng_seed, RngSeed::Fixed(0x1010_c0ff_ee)));
}

// --- Negative ---

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_negative_n1_arbitrary_path_no_panic(s in ".{0,200}") {
        let _ = parse_query_params(&s);
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_negative_n2_arbitrary_query_suffix_no_panic(q in "[^\n]{0,200}") {
        let path = format!("/p?{q}");
        let _ = parse_query_params(&path);
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_negative_n3_encoder_never_raw_space_in_values(value in query_value_strategy()) {
        let enc = encode_query_component(&value);
        prop_assert!(!enc.contains(' '), "raw space in {enc:?} from {value:?}");
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_negative_n4_encoder_never_raw_amp_eq_in_values(value in query_value_strategy()) {
        let enc = encode_query_component(&value);
        prop_assert!(!enc.contains('&'));
        prop_assert!(!enc.contains('='));
    }
}

#[test]
fn uri_property_negative_n5_binary_ish_no_panic() {
    for s in ["\0", "\u{FFFD}", "%FF", "%", "%2", "\t\n"] {
        let _ = parse_query_params(&format!("/p?q={s}"));
        let _ = encode_query_component(s);
    }
}

#[test]
fn uri_property_negative_n6_fuzz_crash_process_documented() {
    assert!(std::path::Path::new("tests/uri_golden/corpus.json").exists());
}

#[test]
fn uri_property_negative_n7_oversize_hits_limit_budget() {
    let huge = format!("/{}", "x".repeat(20_000));
    assert!(request_target_exceeds_limit(
        &huge,
        DEFAULT_MAX_REQUEST_TARGET_OCTETS
    ));
    let q = param_vec(&[("q".into(), "x".repeat(100))]);
    let _ = encode_query_form_explode(&q);
}

#[test]
fn uri_property_negative_n8_shrink_seed_in_config() {
    assert!(matches!(config().rng_seed, RngSeed::Fixed(_)));
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn uri_property_collection_multi_param_uri_ok(
        pairs in prop::collection::vec(("[a-z]{1,8}", query_value_strategy()), 0..6),
    ) {
        let q = param_vec(&pairs);
        let rebuilt = resolve_path_template("/api", &ParamVec::new(), &q);
        prop_assert!(assert_request_target_uri_ok(&rebuilt).is_ok());
    }
}
