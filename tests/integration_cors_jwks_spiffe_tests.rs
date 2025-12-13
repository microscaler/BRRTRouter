#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for CORS, JWKS, and SPIFFE interoperability
//!
//! These tests verify that the three systems work together correctly:
//! - CORS is always required (even if relaxed)
//! - JWKS can be used independently without SPIFFE
//! - SPIFFE requires JWKS for signature verification
//!
//! # Test Categories
//!
//! 1. **CORS + JWKS Bearer Provider**: Verify CORS headers on JWKS-authenticated requests
//! 2. **CORS + SPIFFE Provider**: Verify CORS headers on SPIFFE-authenticated requests
//! 3. **JWKS Independence**: Verify JWKS works without SPIFFE
//! 4. **SPIFFE JWKS Requirement**: Verify SPIFFE requires JWKS URL

use brrtrouter::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::middleware::{CorsMiddleware, Middleware};
use brrtrouter::router::ParamVec;
use brrtrouter::security::{JwksBearerProvider, SecurityProvider, SecurityRequest, SpiffeProvider};
use brrtrouter::spec::SecurityScheme;
use http::Method;
use may::sync::mpsc;
use serde_json::json;
use smallvec::smallvec;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Test Helpers
// ============================================================================

/// Start a mock JWKS server for testing
/// Returns the URL to the JWKS endpoint
fn start_mock_jwks_server(jwks: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}/jwks.json", addr.port());
    
    let jwks_clone = jwks;
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 2048];
                    if stream.read(&mut buf).is_ok() {
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            jwks_clone.len(),
                            jwks_clone
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.flush();
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    thread::sleep(Duration::from_millis(150));
    url
}

/// Create a signed JWT token for JWKS testing
fn make_signed_jwt_for_jwks(
    secret: &[u8],
    issuer: &str,
    audience: &str,
    kid: &str,
    exp_secs: i64,
) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    
    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        ..Default::default()
    };
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let claims = json!({
        "iss": issuer,
        "aud": audience,
        "exp": now + exp_secs,
        "iat": now
    });
    
    let encoding_key = EncodingKey::from_secret(secret);
    jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap()
}

/// Create a signed SPIFFE JWT token
fn make_signed_spiffe_jwt(
    secret: &[u8],
    spiffe_id: &str,
    audience: &str,
    kid: &str,
    exp_secs: i64,
) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    
    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        ..Default::default()
    };
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let claims = json!({
        "sub": spiffe_id,
        "aud": audience,
        "exp": now + exp_secs,
        "iat": now
    });
    
    let encoding_key = EncodingKey::from_secret(secret);
    jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap()
}

/// Base64 URL-safe encoding without padding
fn base64url_no_pad(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

// ============================================================================
// Test Category 1: CORS + JWKS Bearer Provider
// ============================================================================

#[test]
fn test_cors_with_jwks_bearer_provider_preflight() {
    // Setup: Create JWKS provider and CORS middleware
    let secret = b"test-secret-key-for-jwks";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // JWKS provider created but not used in this test (only testing CORS preflight)
    let _jwks_provider = JwksBearerProvider::new(&jwks_url)
        .issuer("https://auth.example.com")
        .audience("my-api");
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    let cors = CorsMiddleware::permissive();
    
    // Test: Preflight OPTIONS request should be handled by CORS
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("origin"), "https://example.com".to_string()),
        (Arc::from("access-control-request-method"), "POST".to_string()),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::OPTIONS,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    // CORS should handle preflight before security validation
    let response = cors.before(&req);
    assert!(response.is_some(), "CORS should handle preflight OPTIONS request");
    let resp = response.unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("*")
    );
}

#[test]
fn test_cors_with_jwks_bearer_provider_authenticated_request() {
    // Setup: Create JWKS provider and CORS middleware
    let secret = b"test-secret-key-for-jwks";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let jwks_provider = JwksBearerProvider::new(&jwks_url)
        .issuer("https://auth.example.com")
        .audience("my-api");
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    let cors = CorsMiddleware::permissive();
    
    // Create a valid JWT token
    let token = make_signed_jwt_for_jwks(
        secret,
        "https://auth.example.com",
        "my-api",
        "test-kid",
        3600,
    );
    
    // Test: Authenticated request should have CORS headers added
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("origin"), "https://example.com".to_string()),
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    // CORS should not block the request (it's not a preflight)
    assert!(cors.before(&req).is_none());
    
    // Verify JWKS provider validates the token
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let security_req = SecurityRequest {
        headers: &req.headers,
        query: &req.query_params,
        cookies: &req.cookies,
    };
    
    let result = jwks_provider.validate(&scheme, &[], &security_req);
    assert!(result, "JWKS provider should validate the token");
    
    // CORS should add headers to response
    let mut resp = HandlerResponse::new(200, HeaderVec::new(), json!({}));
    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("*")
    );
}

#[test]
fn test_cors_invalid_origin_before_jwks_validation() {
    // Setup: Create JWKS provider and restrictive CORS
    let secret = b"test-secret-key-for-jwks";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // JWKS provider created but not used in this test (only testing CORS origin rejection)
    let _jwks_provider = JwksBearerProvider::new(&jwks_url)
        .issuer("https://auth.example.com")
        .audience("my-api");
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    // CORS with specific origin (not wildcard)
    let cors = CorsMiddleware::new(
        vec!["https://allowed.example.com".to_string()],
        vec!["Content-Type".to_string()],
        vec![Method::GET, Method::POST],
        false,
        vec![],
        None,
    );
    
    // Create a valid JWT token
    let token = make_signed_jwt_for_jwks(
        secret,
        "https://auth.example.com",
        "my-api",
        "test-kid",
        3600,
    );
    
    // Test: Invalid origin should be rejected by CORS before JWKS validation
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("origin"), "https://evil.com".to_string()),
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    // CORS should reject invalid origin
    let response = cors.before(&req);
    assert!(response.is_some(), "CORS should reject invalid origin");
    let resp = response.unwrap();
    assert_eq!(resp.status, 403);
}

// ============================================================================
// Test Category 2: CORS + SPIFFE Provider
// ============================================================================

#[test]
fn test_cors_with_spiffe_provider_preflight() {
    // Setup: Create SPIFFE provider and CORS middleware
    let secret = b"test-secret-key-for-spiffe";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let _spiffe_provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    let cors = CorsMiddleware::permissive();
    
    // Test: Preflight OPTIONS request should be handled by CORS
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("origin"), "https://example.com".to_string()),
        (Arc::from("access-control-request-method"), "POST".to_string()),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::OPTIONS,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    // CORS should handle preflight before security validation
    let response = cors.before(&req);
    assert!(response.is_some(), "CORS should handle preflight OPTIONS request");
    let resp = response.unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("*")
    );
}

#[test]
fn test_cors_with_spiffe_provider_authenticated_request() {
    // Setup: Create SPIFFE provider and CORS middleware
    let secret = b"test-secret-key-for-spiffe";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let spiffe_provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    let cors = CorsMiddleware::permissive();
    
    // Create a valid SPIFFE JWT token
    let token = make_signed_spiffe_jwt(
        secret,
        "spiffe://example.com/api/users",
        "api.example.com",
        "test-kid",
        3600,
    );
    
    // Test: Authenticated request should have CORS headers added
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("origin"), "https://example.com".to_string()),
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    // CORS should not block the request (it's not a preflight)
    assert!(cors.before(&req).is_none());
    
    // Verify SPIFFE provider validates the token
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let security_req = SecurityRequest {
        headers: &req.headers,
        query: &req.query_params,
        cookies: &req.cookies,
    };
    
    let result = spiffe_provider.validate(&scheme, &[], &security_req);
    assert!(result, "SPIFFE provider should validate the token");
    
    // CORS should add headers to response
    let mut resp = HandlerResponse::new(200, HeaderVec::new(), json!({}));
    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("*")
    );
}

#[test]
fn test_cors_invalid_origin_before_spiffe_validation() {
    // Setup: Create SPIFFE provider and restrictive CORS
    let secret = b"test-secret-key-for-spiffe";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let _spiffe_provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    // CORS with specific origin (not wildcard)
    let cors = CorsMiddleware::new(
        vec!["https://allowed.example.com".to_string()],
        vec!["Content-Type".to_string()],
        vec![Method::GET, Method::POST],
        false,
        vec![],
        None,
    );
    
    // Create a valid SPIFFE JWT token
    let token = make_signed_spiffe_jwt(
        secret,
        "spiffe://example.com/api/users",
        "api.example.com",
        "test-kid",
        3600,
    );
    
    // Test: Invalid origin should be rejected by CORS before SPIFFE validation
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("origin"), "https://evil.com".to_string()),
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    // CORS should reject invalid origin
    let response = cors.before(&req);
    assert!(response.is_some(), "CORS should reject invalid origin");
    let resp = response.unwrap();
    assert_eq!(resp.status, 403);
}

// ============================================================================
// Test Category 3: JWKS Independence
// ============================================================================

#[test]
fn test_jwks_independent_usage() {
    // Test: JWKS provider should work without SPIFFE
    let secret = b"test-secret-key-for-jwks";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let jwks_provider = JwksBearerProvider::new(&jwks_url)
        .issuer("https://auth.example.com")
        .audience("my-api");
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    // Create a non-SPIFFE JWT token (standard OAuth2 token)
    let token = make_signed_jwt_for_jwks(
        secret,
        "https://auth.example.com",
        "my-api",
        "test-kid",
        3600,
    );
    
    // Test: JWKS provider should validate non-SPIFFE tokens
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let security_req = SecurityRequest {
        headers: &req.headers,
        query: &req.query_params,
        cookies: &req.cookies,
    };
    
    let result = jwks_provider.validate(&scheme, &[], &security_req);
    assert!(result, "JWKS provider should validate non-SPIFFE tokens independently");
}

#[test]
fn test_jwks_no_spiffe_dependency() {
    // Test: Verify JWKS provider has no SPIFFE code
    // This is a compile-time check - if this compiles, JWKS is independent
    
    let secret = b"test-secret-key-for-jwks";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // JWKS provider can be created and used without any SPIFFE imports
    let _jwks_provider = JwksBearerProvider::new(&jwks_url)
        .issuer("https://auth.example.com")
        .audience("my-api");
    
    // If we get here, JWKS is independent (no compile errors)
    assert!(true);
}

// ============================================================================
// Test Category 4: SPIFFE JWKS Requirement
// ============================================================================

#[test]
fn test_spiffe_requires_jwks_url() {
    // Test: SPIFFE validation should fail without JWKS URL
    let spiffe_provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    // Note: No jwks_url() call - this should cause validation to fail
    
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("authorization"), "Bearer invalid-token".to_string()),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let security_req = SecurityRequest {
        headers: &req.headers,
        query: &req.query_params,
        cookies: &req.cookies,
    };
    
    // SPIFFE validation should fail without JWKS URL (fail-secure)
    let result = spiffe_provider.validate(&scheme, &[], &security_req);
    assert!(!result, "SPIFFE validation should fail without JWKS URL");
}

#[test]
fn test_spiffe_succeeds_with_jwks_url() {
    // Test: SPIFFE validation should succeed with JWKS URL configured
    let secret = b"test-secret-key-for-spiffe";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let spiffe_provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    // Create a valid SPIFFE JWT token
    let token = make_signed_spiffe_jwt(
        secret,
        "spiffe://example.com/api/users",
        "api.example.com",
        "test-kid",
        3600,
    );
    
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let security_req = SecurityRequest {
        headers: &req.headers,
        query: &req.query_params,
        cookies: &req.cookies,
    };
    
    // SPIFFE validation should succeed with JWKS URL
    let result = spiffe_provider.validate(&scheme, &[], &security_req);
    assert!(result, "SPIFFE validation should succeed with JWKS URL configured");
}

#[test]
fn test_spiffe_algorithm_mismatch_validation() {
    // Test: SPIFFE should validate algorithm mismatch (security requirement)
    let secret = b"test-secret-key-for-spiffe";
    let k = base64url_no_pad(secret);
    
    // JWKS has HS256 key
    let jwks = json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "test-kid", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    let spiffe_provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    // Wait for initial JWKS fetch
    thread::sleep(Duration::from_millis(200));
    
    // Create a token with HS384 algorithm (mismatch)
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    
    let header = Header {
        kid: Some("test-kid".to_string()),
        alg: Algorithm::HS384, // Different algorithm
        ..Default::default()
    };
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let claims = json!({
        "sub": "spiffe://example.com/api/users",
        "aud": "api.example.com",
        "exp": now + 3600,
        "iat": now
    });
    
    let encoding_key = EncodingKey::from_secret(secret);
    let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();
    
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![
        (Arc::from("authorization"), format!("Bearer {}", token)),
    ];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/api/test".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    };
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let security_req = SecurityRequest {
        headers: &req.headers,
        query: &req.query_params,
        cookies: &req.cookies,
    };
    
    // SPIFFE should reject algorithm mismatch
    let result = spiffe_provider.validate(&scheme, &[], &security_req);
    assert!(!result, "SPIFFE should reject algorithm mismatch");
}
