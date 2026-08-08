//! Story 11.4 — consumer guide + Accept-Query docs fixtures.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::http::{format_accept_query, parse_accept_query, ACCEPT_QUERY_HEADER};
use brrtrouter::spec::load_spec;
use std::path::PathBuf;

const GUIDE: &str = include_str!(
    "../docs/EPICS/URI_REQUEST_TARGET/epic-11-http-query-method/consumer-guide-query-method.md"
);

fn fixture_openapi() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openapi_query_method.yaml")
}

/// P1 — docs OpenAPI fixture loads.
#[test]
fn query_guide_positive_p1_openapi_fixture_loads() {
    let (routes, _) = load_spec(fixture_openapi().to_str().unwrap()).expect("P1");
    assert!(routes.iter().any(|r| r.method.as_str() == "QUERY"));
}

/// P2 — Accept-Query advertise helper.
#[test]
fn query_guide_positive_p2_accept_query_header() {
    let v = format_accept_query(&["application/json"]);
    assert_eq!(ACCEPT_QUERY_HEADER, "Accept-Query");
    assert_eq!(v, "application/json");
    assert_eq!(parse_accept_query(&v), vec!["application/json"]);
}

/// P4 — guide requires uppercase QUERY in fetch.
#[test]
fn query_guide_positive_p4_uppercase_query_in_fetch() {
    assert!(
        GUIDE.contains("method: \"QUERY\""),
        "P4 fetch snippet must use uppercase QUERY"
    );
}

/// P5 — guide links Epic 10 for GET query strings.
#[test]
fn query_guide_positive_p5_epic10_required() {
    assert!(
        GUIDE.contains("Epic 10 is still required for GET query strings"),
        "P5"
    );
    assert!(GUIDE.contains("../BUILD_BOARD.md") || GUIDE.contains("Epic 10"));
}

/// P6 — CORS preflight snippet mentions QUERY Allow-Methods.
#[test]
fn query_guide_positive_p6_cors_preflight_snippet() {
    assert!(GUIDE.contains("Access-Control-Request-Method: QUERY"));
    assert!(GUIDE.contains("Access-Control-Allow-Methods:"));
    assert!(GUIDE.contains("QUERY"));
}

/// N1 — must not recommend lowercase fetch method.
#[test]
fn query_guide_negative_n1_no_lowercase_fetch_recommendation() {
    assert!(
        !GUIDE.contains("method: \"query\""),
        "N1 must not recommend lowercase query in fetch"
    );
}

/// N2 — HTML forms unsupported.
#[test]
fn query_guide_negative_n2_html_forms_unsupported() {
    assert!(GUIDE.to_ascii_lowercase().contains("unsupported"));
    assert!(GUIDE.contains("<form method=\"QUERY\">") || GUIDE.contains("HTML forms"));
}

/// N5 — cache limitations stated.
#[test]
fn query_guide_negative_n5_cache_incomplete() {
    assert!(
        GUIDE.contains("incomplete") || GUIDE.contains("Not implemented"),
        "N5 cache caveats"
    );
}

/// N6 — edge 405 + POST fallback guidance.
#[test]
fn query_guide_negative_n6_post_fallback_on_405() {
    assert!(GUIDE.contains("405"));
    assert!(GUIDE.contains("POST fallback") || GUIDE.contains("Query-Method"));
    assert!(
        GUIDE.contains("Never silently downgrade QUERY → GET") || GUIDE.contains("QUERY → GET")
    );
}
