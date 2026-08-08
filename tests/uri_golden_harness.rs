//! Epic 10 Story 10.1 — URI golden corpus harness.
//!
//! Loads `tests/uri_golden/corpus.json` and asserts inbound `parse_query_params`
//! and outbound `resolve_path_template` behaviour. Every P*/N* vector has a
//! named unit test so CI failures point at the story ID.
//!
//! Matrix: `docs/EPICS/URI_REQUEST_TARGET/compliance-matrix.md`

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use brrtrouter::http::resolve_path_template;
use brrtrouter::router::ParamVec;
use brrtrouter::server::request::parse_query_params;
use brrtrouter::server::request_target::assert_request_target_uri_ok;
use http_legacy::Uri;
use serde::Deserialize;

const CORPUS: &str = include_str!("uri_golden/corpus.json");

#[derive(Debug, Deserialize)]
struct Corpus {
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize, Clone)]
struct Vector {
    id: String,
    #[serde(default)]
    requirement_ids: Vec<String>,
    kind: String,
    #[serde(default)]
    inbound_path: Option<String>,
    #[serde(default)]
    expect_params: Option<Vec<(String, String)>>,
    #[serde(default)]
    rebuild_template: Option<String>,
    #[serde(default)]
    path_params: Option<Vec<(String, String)>>,
    #[serde(default)]
    logical_query: Option<Vec<(String, String)>>,
    #[serde(default)]
    expect_rebuild: Option<String>,
    #[serde(default)]
    expect_uri_ok: Option<bool>,
    #[serde(default)]
    legacy_unencoded_must_fail_uri: Option<bool>,
    #[serde(default)]
    legacy_corrupts_param_count: Option<bool>,
    #[serde(default)]
    legacy_hash_truncates: Option<bool>,
    #[serde(default)]
    story: Option<String>,
    #[serde(default)]
    flag: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

fn load_corpus() -> Corpus {
    serde_json::from_str(CORPUS).expect("uri_golden/corpus.json must parse")
}

fn corpus_by_id() -> BTreeMap<String, Vector> {
    load_corpus()
        .vectors
        .into_iter()
        .map(|v| (v.id.clone(), v))
        .collect()
}

fn param_vec(pairs: &[(String, String)]) -> ParamVec {
    let mut params = ParamVec::new();
    for (k, v) in pairs {
        params.push((Arc::from(k.as_str()), v.clone()));
    }
    params
}

fn pairs_from_param_vec(params: &ParamVec) -> Vec<(String, String)> {
    params
        .iter()
        .map(|(k, v)| (k.as_ref().to_string(), v.clone()))
        .collect()
}

fn legacy_unencoded_query(path_template: &str, query: &[(String, String)]) -> String {
    let mut path = path_template.to_string();
    if !query.is_empty() {
        path.push('?');
        for (i, (k, v)) in query.iter().enumerate() {
            if i > 0 {
                path.push('&');
            }
            path.push_str(k);
            path.push('=');
            path.push_str(v);
        }
    }
    path
}

fn assert_uri_ok(path: &str) {
    // Story 10.8: both http 1.0 and http_legacy 0.2 must accept (hard gate).
    assert_request_target_uri_ok(path)
        .unwrap_or_else(|e| panic!("dual-stack URI rejected {path:?}: {e}"));
}

fn run_vector(v: &Vector) {
    let reqs = v.requirement_ids.join(",");
    let _notes = v.notes.as_deref().unwrap_or("");
    match v.kind.as_str() {
        "inbound_only" | "inbound_no_panic" => {
            let path = v
                .inbound_path
                .as_deref()
                .unwrap_or_else(|| panic!("{}: missing inbound_path [{}]", v.id, reqs));
            let got = pairs_from_param_vec(&parse_query_params(path));
            let expect = v.expect_params.clone().unwrap_or_default();
            assert_eq!(
                got, expect,
                "{} inbound mismatch [{}] notes={:?}",
                v.id, reqs, v.notes
            );
        }
        "roundtrip_query" => {
            let path = v
                .inbound_path
                .as_deref()
                .unwrap_or_else(|| panic!("{}: missing inbound_path", v.id));
            let got = pairs_from_param_vec(&parse_query_params(path));
            let expect = v
                .expect_params
                .clone()
                .unwrap_or_else(|| panic!("{}: missing expect_params", v.id));
            assert_eq!(got, expect, "{} inbound [{}]", v.id, reqs);

            let template = v
                .rebuild_template
                .as_deref()
                .unwrap_or_else(|| panic!("{}: missing rebuild_template", v.id));
            let rebuilt = resolve_path_template(template, &ParamVec::new(), &param_vec(&expect));
            if let Some(want) = &v.expect_rebuild {
                assert_eq!(&rebuilt, want, "{} rebuild [{}]", v.id, reqs);
            }
            if v.expect_uri_ok.unwrap_or(true) {
                assert_uri_ok(&rebuilt);
            }
        }
        "rebuild_path" => {
            let template = v
                .rebuild_template
                .as_deref()
                .unwrap_or_else(|| panic!("{}: missing rebuild_template", v.id));
            let path_params = v
                .path_params
                .clone()
                .unwrap_or_else(|| panic!("{}: missing path_params", v.id));
            let rebuilt =
                resolve_path_template(template, &param_vec(&path_params), &ParamVec::new());
            if let Some(want) = &v.expect_rebuild {
                assert_eq!(&rebuilt, want, "{} path rebuild [{}]", v.id, reqs);
            }
            if v.expect_uri_ok.unwrap_or(true) {
                assert_uri_ok(&rebuilt);
            }
        }
        "legacy_vs_encode" => {
            let template = v
                .rebuild_template
                .as_deref()
                .unwrap_or_else(|| panic!("{}: missing rebuild_template", v.id));
            let logical = v
                .logical_query
                .clone()
                .unwrap_or_else(|| panic!("{}: missing logical_query", v.id));
            let legacy = legacy_unencoded_query(template, &logical);
            if v.legacy_unencoded_must_fail_uri.unwrap_or(false) {
                assert!(
                    legacy.parse::<Uri>().is_err(),
                    "{}: legacy unencoded must fail Uri parse: {legacy:?} [{}]",
                    v.id,
                    reqs
                );
            }
            if v.legacy_corrupts_param_count.unwrap_or(false) {
                // Raw `&` / `=` in value invents extra query pairs when parsed as form.
                let legacy_params = pairs_from_param_vec(&parse_query_params(&legacy));
                assert!(
                    legacy_params.len() > logical.len(),
                    "{}: expected legacy corruption (more params than logical): got {legacy_params:?} [{}]",
                    v.id,
                    reqs
                );
            }
            if v.legacy_hash_truncates.unwrap_or(false) {
                assert!(
                    legacy.contains('#'),
                    "{}: legacy fixture must contain raw '#' [{}]",
                    v.id,
                    reqs
                );
                // http::Uri treats `#` as fragment delimiter on the path-query form.
                if let Ok(uri) = legacy.parse::<Uri>() {
                    let q = uri.query().unwrap_or("");
                    assert!(
                        !q.contains("frag") || q == "k=x",
                        "{}: legacy '#' should truncate query before fragment; query={q:?} [{}]",
                        v.id,
                        reqs
                    );
                }
            }

            let rebuilt = resolve_path_template(template, &ParamVec::new(), &param_vec(&logical));
            if let Some(want) = &v.expect_rebuild {
                assert_eq!(&rebuilt, want, "{} encoded rebuild [{}]", v.id, reqs);
            }
            if v.expect_uri_ok.unwrap_or(true) {
                assert_uri_ok(&rebuilt);
            }
            // Encoded value must not re-introduce raw delimiters in the value side.
            assert!(
                !rebuilt.contains(' '),
                "{}: encoded rebuild must not contain raw space: {rebuilt:?}",
                v.id
            );
        }
        "deferred" => {
            assert_eq!(
                v.flag.as_deref(),
                Some("length"),
                "{}: deferred length flag [{}] story={:?}",
                v.id,
                reqs,
                v.story
            );
            assert_eq!(v.story.as_deref(), Some("10.6"));
        }
        other => panic!("{}: unknown kind {other:?} [{}]", v.id, reqs),
    }
}

fn run_id(id: &str) {
    let map = corpus_by_id();
    let v = map
        .get(id)
        .unwrap_or_else(|| panic!("missing golden vector {id}"));
    run_vector(v);
}

#[test]
fn uri_golden_corpus_file_exists_on_disk() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/uri_golden/corpus.json");
    assert!(path.is_file(), "missing {}", path.display());
}

#[test]
fn uri_golden_corpus_requirement_ids_nonempty_except_deferred_ok() {
    for v in load_corpus().vectors {
        assert!(
            !v.requirement_ids.is_empty(),
            "{} must map to matrix Requirement ID(s)",
            v.id
        );
    }
}

#[test]
fn uri_golden_corpus_runs_all_vectors() {
    for v in load_corpus().vectors {
        run_vector(&v);
    }
}

// --- Positive (named per TESTING_STANDARD) ---

#[test]
fn uri_golden_positive_p1_ascii_kv() {
    run_id("P1");
}

#[test]
fn uri_golden_positive_p2_percent20_space() {
    run_id("P2");
}

#[test]
fn uri_golden_positive_p3_plus_space() {
    run_id("P3");
}

#[test]
fn uri_golden_positive_p4_accented() {
    run_id("P4");
}

#[test]
fn uri_golden_positive_p5_cjk_emoji() {
    run_id("P5");
}

#[test]
fn uri_golden_positive_p6_duplicate_keys() {
    run_id("P6");
}

#[test]
fn uri_golden_positive_p7_empty_value() {
    run_id("P7");
}

#[test]
fn uri_golden_positive_p8_unreserved() {
    run_id("P8");
}

#[test]
fn uri_golden_positive_p9_path_segment_space() {
    run_id("P9");
}

#[test]
fn uri_golden_positive_p10_multi_param() {
    run_id("P10");
}

// --- Negative ---

#[test]
fn uri_golden_negative_n1_truncated_percent() {
    run_id("N1");
    run_id("N1b");
}

#[test]
fn uri_golden_negative_n2_illegal_hex() {
    run_id("N2");
}

#[test]
fn uri_golden_negative_n3_raw_space_needs_encode() {
    run_id("N3");
}

#[test]
fn uri_golden_negative_n4_raw_ampersand_equals() {
    run_id("N4");
}

#[test]
fn uri_golden_negative_n5_raw_hash() {
    run_id("N5");
}

#[test]
fn uri_golden_negative_n6_control_chars() {
    run_id("N6");
}

#[test]
fn uri_golden_negative_n7_oversize_deferred_to_10_6() {
    run_id("N7");
}

#[test]
fn uri_golden_negative_n8_missing_query() {
    run_id("N8");
}
