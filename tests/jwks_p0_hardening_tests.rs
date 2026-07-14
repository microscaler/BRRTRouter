//! P0 JWT hardening tests for configured algorithms and unknown-key rotation.
//!
//! These tests cover the shared BRRTRouter behavior required by Sesame-IDAM P0:
//! trusted algorithm allow-lists, immediate refresh after an unknown `kid`, and a
//! cooldown that bounds attacker-triggered JWKS requests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::router::ParamVec;
use brrtrouter::security::{JwksBearerProvider, JwtTokenStatus, SecurityProvider, SecurityRequest};
use brrtrouter::spec::SecurityScheme;
use jsonwebtoken::{Algorithm, EncodingKey, Header};

fn bearer_scheme() -> SecurityScheme {
    SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: Some("JWT".to_string()),
        description: None,
    }
}

fn token(secret: &[u8], kid: &str) -> String {
    let header = Header {
        alg: Algorithm::HS256,
        kid: Some(kid.to_string()),
        typ: Some("at+jwt".to_string()),
        ..Header::default()
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = serde_json::json!({
        "sub": "test-subject",
        "jti": format!("jti-{kid}"),
        "ver": 1,
        "exp": now + 300,
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

fn jwks(secret: &[u8], kid: &str) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
    serde_json::json!({
        "keys": [{
            "kty": "oct",
            "alg": "HS256",
            "kid": kid,
            "k": encoded,
        }]
    })
    .to_string()
}

fn request(token: &str) -> (HeaderVec, ParamVec, HeaderVec) {
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    (headers, ParamVec::new(), HeaderVec::new())
}

fn validate(provider: &JwksBearerProvider, token: &str) -> bool {
    let (headers, query, cookies) = request(token);
    let request = SecurityRequest {
        headers: &headers,
        query: &query,
        cookies: &cookies,
    };
    provider.validate(&bearer_scheme(), &[], &request)
}

fn start_jwks_server(first: String, subsequent: String) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request_count = Arc::new(AtomicUsize::new(0));
    let server_count = Arc::clone(&request_count);

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut request_bytes = [0_u8; 2048];
            let _ = stream.read(&mut request_bytes);
            let index = server_count.fetch_add(1, Ordering::SeqCst);
            let body = if index == 0 { &first } else { &subsequent };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (
        format!("http://{address}/.well-known/jwks.json"),
        request_count,
    )
}

#[test]
fn configured_asymmetric_allow_list_rejects_hmac_before_jwks_fetch() {
    let provider = JwksBearerProvider::new("http://127.0.0.1:1/jwks.json")
        .allowed_algorithms(&[Algorithm::EdDSA]);
    provider.stop_background_refresh();

    assert!(!validate(&provider, &token(b"test-secret", "hmac-key")));
}

#[test]
fn current_asymmetric_algorithm_set_can_be_configured() {
    let provider = JwksBearerProvider::new("http://127.0.0.1:1/jwks.json").allowed_algorithms(&[
        Algorithm::EdDSA,
        Algorithm::ES256,
        Algorithm::ES384,
        Algorithm::PS256,
        Algorithm::PS384,
        Algorithm::PS512,
        Algorithm::RS256,
        Algorithm::RS384,
        Algorithm::RS512,
    ]);
    provider.stop_background_refresh();
}

#[test]
fn unknown_kid_forces_refresh_before_normal_cache_ttl_expires() {
    let first_secret = b"first-test-secret";
    let rotated_secret = b"rotated-test-secret";
    let (url, request_count) =
        start_jwks_server(jwks(first_secret, "kid-1"), jwks(rotated_secret, "kid-2"));
    let provider = JwksBearerProvider::new(url)
        .cache_ttl(Duration::from_secs(300))
        .unknown_kid_refresh_cooldown(Duration::ZERO);
    provider.stop_background_refresh();

    assert!(validate(&provider, &token(first_secret, "kid-1")));
    assert!(validate(&provider, &token(rotated_secret, "kid-2")));
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
}

#[test]
fn unknown_kid_refreshes_are_rate_limited() {
    let secret = b"stable-test-secret";
    let stable_jwks = jwks(secret, "known-kid");
    let (url, request_count) = start_jwks_server(stable_jwks.clone(), stable_jwks);
    let provider = JwksBearerProvider::new(url)
        .cache_ttl(Duration::from_secs(300))
        .unknown_kid_refresh_cooldown(Duration::from_secs(60));
    provider.stop_background_refresh();

    assert!(validate(&provider, &token(secret, "known-kid")));
    assert!(!validate(&provider, &token(secret, "unknown-kid-1")));
    assert!(!validate(&provider, &token(secret, "unknown-kid-2")));
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
}

#[test]
fn dynamic_status_is_rechecked_on_claims_cache_hits() {
    let secret = b"status-check-secret";
    let stable_jwks = jwks(secret, "status-kid");
    let (url, _) = start_jwks_server(stable_jwks.clone(), stable_jwks);
    let revoked = Arc::new(AtomicBool::new(false));
    let checker_state = Arc::clone(&revoked);
    let checker = Arc::new(move |_claims: &serde_json::Value| {
        if checker_state.load(Ordering::SeqCst) {
            JwtTokenStatus::Revoked
        } else {
            JwtTokenStatus::Active
        }
    });
    let provider = JwksBearerProvider::new(url).token_status_checker(checker);
    provider.stop_background_refresh();
    let access_token = token(secret, "status-kid");

    assert!(validate(&provider, &access_token));
    revoked.store(true, Ordering::SeqCst);
    assert!(!validate(&provider, &access_token));
}

#[test]
fn unavailable_dynamic_status_fails_closed() {
    let secret = b"unavailable-secret";
    let stable_jwks = jwks(secret, "unavailable-kid");
    let (url, _) = start_jwks_server(stable_jwks.clone(), stable_jwks);
    let checker = Arc::new(|_claims: &serde_json::Value| JwtTokenStatus::Unavailable);
    let provider = JwksBearerProvider::new(url).token_status_checker(checker);
    provider.stop_background_refresh();

    assert!(!validate(&provider, &token(secret, "unavailable-kid")));
}

#[test]
fn dynamic_status_is_checked_once_before_claims_extraction() {
    let secret = b"single-status-check-secret";
    let stable_jwks = jwks(secret, "single-status-kid");
    let (url, _) = start_jwks_server(stable_jwks.clone(), stable_jwks);
    let checks = Arc::new(AtomicUsize::new(0));
    let checker_count = Arc::clone(&checks);
    let checker = Arc::new(move |_claims: &serde_json::Value| {
        checker_count.fetch_add(1, Ordering::SeqCst);
        JwtTokenStatus::Active
    });
    let provider = JwksBearerProvider::new(url).token_status_checker(checker);
    provider.stop_background_refresh();
    let access_token = token(secret, "single-status-kid");
    let (headers, query, cookies) = request(&access_token);
    let security_request = SecurityRequest {
        headers: &headers,
        query: &query,
        cookies: &cookies,
    };
    let scheme = bearer_scheme();

    assert!(provider.validate(&scheme, &[], &security_request));
    assert!(provider
        .extract_claims(&scheme, &security_request)
        .is_some());
    assert_eq!(checks.load(Ordering::SeqCst), 1);
}
