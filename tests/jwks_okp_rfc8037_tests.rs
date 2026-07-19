//! OKP/Ed25519 (EdDSA) JWKS round-trip tests — the regression guard for an
//! RFC 8037 key-casing incident observed against a production IdAM.
//!
//! Every case signs a REAL EdDSA token and serves a JWKS over HTTP, then
//! drives the actual `JwksBearerProvider::validate` path:
//!   * RFC-8037-cased key ("OKP"/"Ed25519") verifies the token.
//!   * `alg`-less OKP key verifies (RFC 8037 makes `alg` OPTIONAL).
//!   * mis-cased `kty` ("okp") / `crv` ("ED25519") are REJECTED.
//!   * a contradictory `alg` is REJECTED.
//!
//! The existing OKP fixture in `jwks_headers_integration_tests.rs` only tests
//! response cache headers and omits `alg`, so it would NOT have caught the
//! incident. This one would.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::router::ParamVec;
use brrtrouter::security::{JwksBearerProvider, SecurityProvider, SecurityRequest};
use brrtrouter::spec::SecurityScheme;
use jsonwebtoken::{Algorithm, EncodingKey, Header};

/// RFC 8037 Appendix A.1 Ed25519 test key.
/// `d` (seed) and `x` (public key), base64url — deterministic, no key-gen dep.
const RFC8037_SEED_B64: &str = "nWGxne_9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A";
const RFC8037_X_B64: &str = "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo";

/// Wrap the 32-byte Ed25519 seed in a PKCS#8 v1 document for
/// `EncodingKey::from_ed_der`. The 16-byte prefix is the fixed ASN.1 header
/// for an Ed25519 OneAsymmetricKey (OID 1.3.101.112).
fn ed_encoding_key() -> EncodingKey {
    let seed = URL_SAFE_NO_PAD.decode(RFC8037_SEED_B64).unwrap();
    let mut pkcs8: Vec<u8> = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    pkcs8.extend_from_slice(&seed);
    EncodingKey::from_ed_der(&pkcs8)
}

fn sign_ed(kid: &str) -> String {
    let header = Header {
        alg: Algorithm::EdDSA,
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
    jsonwebtoken::encode(&header, &claims, &ed_encoding_key()).unwrap()
}

/// Build a JWKS document with explicit `kty`/`crv` casing and optional `alg`,
/// so tests can serve both RFC-correct and deliberately-malformed keys.
fn jwks_okp(kid: &str, kty: &str, crv: &str, alg: Option<&str>) -> String {
    let mut key = serde_json::json!({
        "kty": kty,
        "crv": crv,
        "use": "sig",
        "kid": kid,
        "x": RFC8037_X_B64,
    });
    if let Some(a) = alg {
        key["alg"] = serde_json::json!(a);
    }
    serde_json::json!({ "keys": [key] }).to_string()
}

fn start_jwks_server(body: String) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request_count = Arc::new(AtomicUsize::new(0));
    let server_count = Arc::clone(&request_count);
    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0_u8; 2048];
            let _ = stream.read(&mut buf);
            server_count.fetch_add(1, Ordering::SeqCst);
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

fn bearer_scheme() -> SecurityScheme {
    SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: Some("JWT".to_string()),
        description: None,
    }
}

fn validate(provider: &JwksBearerProvider, token: &str) -> bool {
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let query = ParamVec::new();
    let cookies = HeaderVec::new();
    let request = SecurityRequest {
        headers: &headers,
        query: &query,
        cookies: &cookies,
    };
    provider.validate(&bearer_scheme(), &[], &request)
}

fn provider_for(body: String) -> JwksBearerProvider {
    let (url, _count) = start_jwks_server(body);
    let provider = JwksBearerProvider::new(url).allowed_algorithms(&[Algorithm::EdDSA]);
    provider.stop_background_refresh();
    provider
}

#[test]
fn okp_rfc8037_casing_verifies() {
    let provider = provider_for(jwks_okp("ed-kid", "OKP", "Ed25519", Some("EdDSA")));
    assert!(
        validate(&provider, &sign_ed("ed-kid")),
        "an RFC-8037-cased OKP/Ed25519 key must verify a real EdDSA token"
    );
}

#[test]
fn okp_without_alg_verifies() {
    // RFC 8037 makes `alg` OPTIONAL on an OKP JWK.
    let provider = provider_for(jwks_okp("ed-kid", "OKP", "Ed25519", None));
    assert!(
        validate(&provider, &sign_ed("ed-kid")),
        "an OKP key with no `alg` is RFC-legal and must verify"
    );
}

#[test]
fn okp_lowercase_kty_rejected() {
    // The original incident: kty "okp" instead of "OKP".
    let provider = provider_for(jwks_okp("ed-kid", "okp", "Ed25519", Some("EdDSA")));
    assert!(
        !validate(&provider, &sign_ed("ed-kid")),
        "non-RFC kty casing (\"okp\") must be rejected, not tolerated"
    );
}

#[test]
fn okp_uppercase_crv_rejected() {
    // The original incident: crv "ED25519" instead of "Ed25519".
    let provider = provider_for(jwks_okp("ed-kid", "OKP", "ED25519", Some("EdDSA")));
    assert!(
        !validate(&provider, &sign_ed("ed-kid")),
        "non-RFC crv casing (\"ED25519\") must be rejected"
    );
}

#[test]
fn okp_contradictory_alg_rejected() {
    let provider = provider_for(jwks_okp("ed-kid", "OKP", "Ed25519", Some("RS256")));
    assert!(
        !validate(&provider, &sign_ed("ed-kid")),
        "an OKP key with a non-EdDSA `alg` is contradictory and must be rejected"
    );
}
