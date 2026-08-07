//! Story 10.11 — may_minihttp / httparse → BRRTRouter request-target contract.
//!
//! Simulates the front parser (`httparse`, as used by may_minihttp::request::decode)
//! then feeds `Request::path()`-equivalent strings into app helpers.

use brrtrouter::server::request::parse_query_params;
use brrtrouter::server::{path_only, request_target_for_app};

fn httparse_path(request_line: &str) -> Result<String, String> {
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);
    let buf = format!("{request_line}\r\nHost: example.com\r\n\r\n");
    match req.parse(buf.as_bytes()) {
        Ok(httparse::Status::Complete(_)) => Ok(req.path.unwrap_or("").to_string()),
        Ok(httparse::Status::Partial) => Err("partial".into()),
        Err(e) => Err(format!("{e:?}")),
    }
}

fn find_q(path: &str, name: &str) -> Option<String> {
    parse_query_params(path)
        .iter()
        .find(|(k, _)| k.as_ref() == name)
        .map(|(_, v)| v.clone())
}

#[test]
fn boundary_positive_p1_origin_form_to_query_params() {
    let raw = httparse_path("GET /p?q=1 HTTP/1.1").unwrap();
    assert_eq!(raw, "/p?q=1");
    let app = request_target_for_app(&raw);
    assert_eq!(app, "/p?q=1");
    assert_eq!(path_only(app), "/p");
    assert_eq!(find_q(app, "q").as_deref(), Some("1"));
}

#[test]
fn boundary_positive_p2_encoded_space() {
    let raw = httparse_path("GET /p?q=South%20Africa HTTP/1.1").unwrap();
    assert_eq!(raw, "/p?q=South%20Africa");
    assert_eq!(
        find_q(request_target_for_app(&raw), "q").as_deref(),
        Some("South Africa")
    );
}

#[test]
fn boundary_positive_p3_long_target_accepted_by_front() {
    let long = "x".repeat(8000);
    let line = format!("GET /p?q={long} HTTP/1.1");
    let raw = httparse_path(&line).expect("front accepts long target today");
    assert!(raw.len() > 8000);
    // App still parses; 414 is Story 10.6
    assert_eq!(
        find_q(request_target_for_app(&raw), "q").as_deref(),
        Some(long.as_str())
    );
}

#[test]
fn boundary_positive_p4_absolute_form_normalized() {
    let raw = httparse_path("GET http://example.com/p?q=1 HTTP/1.1").unwrap();
    assert_eq!(raw, "http://example.com/p?q=1");
    let app = request_target_for_app(&raw);
    assert_eq!(app, "/p?q=1");
    assert_eq!(path_only(app), "/p");
    assert_eq!(find_q(app, "q").as_deref(), Some("1"));
}

#[test]
fn boundary_positive_p5_multi_segment() {
    let raw = httparse_path("GET /api/v1/regions/west HTTP/1.1").unwrap();
    assert_eq!(
        path_only(request_target_for_app(&raw)),
        "/api/v1/regions/west"
    );
}

#[test]
fn boundary_positive_p6_plus_and_percent20_octets_preserved() {
    let plus = httparse_path("GET /p?q=South+Africa HTTP/1.1").unwrap();
    let pct = httparse_path("GET /p?q=South%20Africa HTTP/1.1").unwrap();
    assert!(plus.contains('+'));
    assert!(pct.contains("%20"));
    assert_eq!(
        find_q(request_target_for_app(&plus), "q"),
        find_q(request_target_for_app(&pct), "q")
    );
}

#[test]
fn boundary_negative_n1_raw_space_rejected_at_front() {
    let err = httparse_path("GET /p q=1 HTTP/1.1").unwrap_err();
    assert!(
        err.contains("Version") || err.contains("Token"),
        "got {err}"
    );
}

#[test]
fn boundary_negative_n2_ctl_tab_rejected_at_front() {
    let err = httparse_path("GET /p?q=a\tb HTTP/1.1").unwrap_err();
    assert!(err.contains("Token"), "got {err}");
}

#[test]
fn boundary_negative_n3_oversize_deferred_note() {
    // Front accepts; length enforcement is Story 10.6 (documented in boundary doc).
    let line = format!("GET /p?q={} HTTP/1.1", "y".repeat(9000));
    assert!(httparse_path(&line).is_ok());
}

#[test]
fn boundary_negative_n4_nul_rejected_at_front() {
    let err = httparse_path("GET /p?q=a\0b HTTP/1.1").unwrap_err();
    assert!(err.contains("Token"), "got {err}");
}

#[test]
fn boundary_negative_n5_hash_passthrough() {
    let raw = httparse_path("GET /p?q=a#frag HTTP/1.1").unwrap();
    assert_eq!(raw, "/p?q=a#frag");
    assert_eq!(request_target_for_app(&raw), "/p?q=a#frag");
}

#[test]
fn boundary_negative_n6_absolute_form_host_only() {
    let raw = httparse_path("GET http://example.com HTTP/1.1").unwrap();
    assert_eq!(request_target_for_app(&raw), "/");
}

#[test]
fn boundary_negative_n7_double_slash_passthrough() {
    let raw = httparse_path("GET //evil/p HTTP/1.1").unwrap();
    assert_eq!(request_target_for_app(&raw), "//evil/p");
}

#[test]
fn boundary_options_asterisk() {
    let raw = httparse_path("OPTIONS * HTTP/1.1").unwrap();
    assert_eq!(raw, "*");
    assert_eq!(request_target_for_app(&raw), "*");
}
