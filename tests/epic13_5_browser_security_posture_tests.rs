//! Epic 13.5 — browser security posture (Option B) fixtures.

use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::security::{SameSite, SetCookieBuilder};
use brrtrouter::server::request::parse_cookies;
use std::sync::Arc;

#[test]
fn epic13_5_p1_posture_doc_picks_option_b() {
    let doc = include_str!("../docs/BROWSER_SECURITY_POSTURE.md");
    assert!(doc.contains("Option B"));
    assert!(doc.contains("Bearer"));
    assert!(
        doc.contains("no server-session framework")
            || doc.contains("does **not** ship cookie-session")
    );
    assert!(
        !doc.contains("Decision: Option A"),
        "must not pick Option A as the shipped decision"
    );
}

#[test]
fn epic13_5_p2b_readme_omits_session_middleware_claim() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("BROWSER_SECURITY_POSTURE") || readme.contains("Bearer/JWKS only"),
        "README should point at browser posture"
    );
    let lower = readme.to_lowercase();
    assert!(
        !lower.contains("session middleware") && !lower.contains("sessions included"),
        "README must not claim session middleware"
    );
}

#[test]
fn epic13_5_p2_builder_secure_defaults() {
    let v = SetCookieBuilder::new("auth_token", "jwt-here")
        .same_site(SameSite::Strict)
        .build();
    assert!(v.contains("HttpOnly"));
    assert!(v.contains("Secure"));
    assert!(v.contains("SameSite=Strict"));
}

#[test]
fn epic13_5_n3_malformed_cookie_no_panic() {
    let mut headers = HeaderVec::new();
    headers.push((
        Arc::from("cookie"),
        ";;;=;;bad\nname=value;=only".to_string(),
    ));
    let cookies = parse_cookies(&headers);
    // Must not panic; may yield partial pairs.
    let _ = cookies.len();
}

#[test]
fn epic13_5_n4_docs_forbid_sessions_included() {
    let doc = include_str!("../docs/BROWSER_SECURITY_POSTURE.md");
    assert!(doc.contains("Forbidden claim") || doc.contains("Forbidden"));
    assert!(
        doc.contains("Sessions included")
            || doc.contains("sessions included")
            || doc.contains("\"Sessions included\"")
    );
}

#[test]
fn epic13_5_n5_builder_no_panic_on_hostile() {
    let _ = SetCookieBuilder::new("x\0y", "a\rb\nc").build();
}

#[test]
fn epic13_5_p5_cookie_name_api_still_documented() {
    // Regression signal: providers still expose cookie_name in public docs/module.
    let sec = include_str!("../src/security/mod.rs");
    assert!(sec.contains("cookie_name"));
    let posture = include_str!("../docs/BROWSER_SECURITY_POSTURE.md");
    assert!(posture.contains("cookie_name"));
}
