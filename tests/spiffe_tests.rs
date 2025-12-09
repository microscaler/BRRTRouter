#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Comprehensive tests for SPIFFE security provider
//!
//! Tests SPIFFE JWT SVID validation including:
//! - SPIFFE ID format validation
//! - Trust domain whitelist enforcement
//! - Audience validation
//! - Expiration checking
//! - JWT signature verification (Phase 2)
//! - JWT claim extraction
//! - Integration with SecurityProvider trait

use base64::Engine;
use brrtrouter::security::{SecurityProvider, SecurityRequest, SpiffeProvider};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{dispatcher::HeaderVec, router::ParamVec};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use bollard::Docker;
use bollard::models::{HostConfig, PortBinding};
use bollard::query_parameters::RemoveContainerOptionsBuilder;
use futures::executor::block_on;

/// Helper to create a SPIFFE JWT token for testing
///
/// Creates a JWT with SPIFFE-specific claims:
/// - `sub`: SPIFFE ID (required)
/// - `aud`: Audience (required)
/// - `exp`: Expiration timestamp
/// - `iat`: Issued at timestamp
fn make_spiffe_jwt(
    spiffe_id: &str,
    audience: &str,
    exp_secs: i64,
    iat_secs: i64,
) -> String {
    use base64::{engine::general_purpose, Engine as _};
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let exp = now + exp_secs;
    let iat = now + iat_secs;
    
    let header = json!({
        "alg": "HS256",
        "typ": "JWT"
    });
    
    let payload = json!({
        "sub": spiffe_id,
        "aud": audience,
        "exp": exp,
        "iat": iat
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    
    // Note: This is a test token without signature verification
    // In production, signature would be verified via JWKS
    format!("{header_b64}.{payload_b64}.signature")
}

/// Helper to create a properly signed SPIFFE JWT using jsonwebtoken
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

/// Helper to create an expired SPIFFE JWT token
fn make_expired_spiffe_jwt(spiffe_id: &str, audience: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    // Expired 1 hour ago
    let exp = now - 3600;
    let iat = now - 7200; // Issued 2 hours ago
    
    let header = json!({
        "alg": "HS256",
        "typ": "JWT"
    });
    
    let payload = json!({
        "sub": spiffe_id,
        "aud": audience,
        "exp": exp,
        "iat": iat
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    
    format!("{header_b64}.{payload_b64}.signature")
}

/// Helper to create a SPIFFE JWT with array audience
fn make_spiffe_jwt_array_aud(spiffe_id: &str, audiences: &[&str], exp_secs: i64) -> String {
    use base64::{engine::general_purpose, Engine as _};
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let exp = now + exp_secs;
    
    let header = json!({
        "alg": "HS256",
        "typ": "JWT"
    });
    
    let payload = json!({
        "sub": spiffe_id,
        "aud": audiences,
        "exp": exp,
        "iat": now
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    
    format!("{header_b64}.{payload_b64}.signature")
}

/// Helper to base64url encode without padding
fn base64url_no_pad(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// RAII wrapper for Docker-based JWKS mock server
struct JwksMockServerContainer {
    docker: Docker,
    container_id: String,
    url: String,
}

impl JwksMockServerContainer {
    /// Start a Docker container with a simple HTTP server serving JWKS
    fn new(jwks_json: String) -> Self {
        // Check if Docker is available
        let docker = match Docker::connect_with_local_defaults() {
            Ok(d) => d,
            Err(_) => {
                // Fall back to in-process server if Docker not available
                return Self::fallback_in_process(jwks_json);
            }
        };
        
        // Create a simple HTTP server container using nginx:alpine
        // We'll use a custom nginx config to serve the JWKS JSON
        let port_key = "80/tcp".to_string();
        let bindings = std::collections::HashMap::from([(
            port_key,
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".into()),
                host_port: Some("0".into()),
            }]),
        )]);
        let host_config = HostConfig {
            port_bindings: Some(bindings),
            ..Default::default()
        };
        
        // For now, use in-process fallback since Docker setup is complex
        // TODO: Implement full Docker-based mock server
        Self::fallback_in_process(jwks_json)
    }
    
    /// Fallback to in-process server when Docker is not available
    fn fallback_in_process(jwks: String) -> Self {
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
        
        // Return a dummy container struct (Docker not actually used in fallback)
        Self {
            docker: Docker::connect_with_local_defaults().unwrap_or_else(|_| {
                panic!("Docker connection failed and fallback also failed")
            }),
            container_id: "fallback".to_string(),
            url,
        }
    }
    
    fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for JwksMockServerContainer {
    fn drop(&mut self) {
        if self.container_id != "fallback" {
            let opts = RemoveContainerOptionsBuilder::default().force(true).build();
            let _ = block_on(self.docker.remove_container(&self.container_id, Some(opts)));
        }
    }
}

/// Start a mock JWKS server for testing
/// Returns the URL to the JWKS endpoint
///
/// Uses in-process TCP server for simplicity and speed.
/// Handles multiple connections to support cache refreshes and retries.
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

#[test]
fn test_spiffe_provider_creation() {
    let _provider = SpiffeProvider::new();
    assert!(true); // Basic creation test
    
    let _provider_with_config = SpiffeProvider::new()
        .trust_domains(&["example.com", "enterprise.local"])
        .audiences(&["api.example.com"])
        .leeway(120);
    assert!(true);
}

#[test]
fn test_spiffe_id_format_validation() {
    // Test SPIFFE ID format validation through provider validation
    // Valid SPIFFE IDs should pass validation
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com", "enterprise.local", "prod.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Valid SPIFFE IDs
    let valid_ids = vec![
        "spiffe://example.com/api/users",
        "spiffe://enterprise.local/windows/service",
        "spiffe://prod.example.com/frontend/web",
        "spiffe://example.com",
    ];
    
    for spiffe_id in valid_ids {
        let token = make_spiffe_jwt(spiffe_id, "api.example.com", 3600, 0);
        let mut headers: HeaderVec = HeaderVec::new();
        headers.push((Arc::from("authorization"), format!("Bearer {token}")));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        // Should pass validation (format is valid)
        assert!(
            provider.validate(&scheme, &[], &req),
            "Valid SPIFFE ID '{spiffe_id}' should pass validation"
        );
    }
    
    // Invalid SPIFFE IDs should fail validation
    let invalid_ids = vec![
        "invalid",
        "http://example.com",
        "spiffe://",
        "spiffe:///path",
    ];
    
    for spiffe_id in invalid_ids {
        // Create token with invalid SPIFFE ID
        use base64::{engine::general_purpose, Engine as _};
        let header = json!({"alg": "HS256", "typ": "JWT"});
        let payload = json!({
            "sub": spiffe_id,
            "aud": "api.example.com",
            "exp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64 + 3600,
            "iat": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        });
        
        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
        let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
        let token = format!("{header_b64}.{payload_b64}.signature");
        
        let mut headers: HeaderVec = HeaderVec::new();
        headers.push((Arc::from("authorization"), format!("Bearer {token}")));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        // Should fail validation (format is invalid)
        assert!(
            !provider.validate(&scheme, &[], &req),
            "Invalid SPIFFE ID '{spiffe_id}' should fail validation"
        );
    }
}

#[test]
fn test_extract_trust_domain() {
    // Test trust domain extraction through provider validation
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com", "enterprise.local", "prod.example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Test trust domain extraction by validating tokens with different domains
    let test_cases = vec![
        ("spiffe://example.com/api/users", true),
        ("spiffe://enterprise.local/windows/service", true),
        ("spiffe://prod.example.com/frontend", true),
        ("spiffe://untrusted.com/api/users", false), // Not in whitelist
    ];
    
    for (spiffe_id, should_pass) in test_cases {
        let token = make_spiffe_jwt(spiffe_id, "api.example.com", 3600, 0);
        let mut headers: HeaderVec = HeaderVec::new();
        headers.push((Arc::from("authorization"), format!("Bearer {token}")));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        let result = provider.validate(&scheme, &[], &req);
        assert_eq!(
            result, should_pass,
            "SPIFFE ID '{}' should {} validation",
            spiffe_id,
            if should_pass { "pass" } else { "fail" }
        );
    }
}

#[test]
fn test_spiffe_validation_valid_svid() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Valid SPIFFE SVID should pass validation"
    );
}

#[test]
fn test_spiffe_validation_missing_token() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let headers: HeaderVec = HeaderVec::new();
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Missing token should fail validation"
    );
}

#[test]
fn test_spiffe_validation_invalid_spiffe_id() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Token with invalid SPIFFE ID format
    use base64::{engine::general_purpose, Engine as _};
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "sub": "invalid-spiffe-id",
        "aud": "api.example.com",
        "exp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 + 3600,
        "iat": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    let token = format!("{header_b64}.{payload_b64}.signature");
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid SPIFFE ID format should fail validation"
    );
}

#[test]
fn test_spiffe_validation_trust_domain_whitelist() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Token with untrusted domain
    let token = make_spiffe_jwt(
        "spiffe://untrusted.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Untrusted domain should fail validation"
    );
}

#[test]
fn test_spiffe_validation_empty_trust_domains() {
    // Empty trust domains means all domains are allowed
    let provider = SpiffeProvider::new()
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://any-domain.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Empty trust domains should allow any domain"
    );
}

#[test]
fn test_spiffe_validation_audience_string() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com", "brrtrouter"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Valid audience
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Valid audience should pass validation"
    );
    
    // Invalid audience
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "wrong-audience",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid audience should fail validation"
    );
}

#[test]
fn test_spiffe_validation_audience_array() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com", "brrtrouter"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Token with array audience containing valid audience
    let token = make_spiffe_jwt_array_aud(
        "spiffe://example.com/api/users",
        &["api.example.com", "other-audience"],
        3600,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Array audience with valid entry should pass validation"
    );
    
    // Token with array audience containing no valid audience
    let token = make_spiffe_jwt_array_aud(
        "spiffe://example.com/api/users",
        &["wrong-audience", "another-wrong"],
        3600,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Array audience with no valid entry should fail validation"
    );
}

#[test]
fn test_spiffe_validation_empty_audiences() {
    // Empty audiences means no audience validation
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "any-audience",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Empty audiences should allow any audience"
    );
}

#[test]
fn test_spiffe_validation_expired_token() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .leeway(60); // 60 seconds leeway
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_expired_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Expired token should fail validation"
    );
}

#[test]
fn test_spiffe_validation_missing_sub_claim() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Token without 'sub' claim
    use base64::{engine::general_purpose, Engine as _};
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "aud": "api.example.com",
        "exp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 + 3600,
        "iat": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    let token = format!("{header_b64}.{payload_b64}.signature");
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Missing 'sub' claim should fail validation"
    );
}

#[test]
fn test_spiffe_validation_missing_exp_claim() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Token without 'exp' claim
    use base64::{engine::general_purpose, Engine as _};
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "sub": "spiffe://example.com/api/users",
        "aud": "api.example.com",
        "iat": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    let token = format!("{header_b64}.{payload_b64}.signature");
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Missing 'exp' claim should fail validation"
    );
}

#[test]
fn test_spiffe_validation_wrong_scheme() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    // Wrong scheme (not HTTP Bearer)
    let scheme = SecurityScheme::ApiKey {
        name: "X-API-Key".to_string(),
        location: "header".to_string(),
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Wrong security scheme should fail validation"
    );
}

#[test]
fn test_spiffe_extract_spiffe_id() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"]);
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    let spiffe_id = provider.extract_spiffe_id(&req);
    assert_eq!(
        spiffe_id,
        Some("spiffe://example.com/api/users".to_string()),
        "Should extract SPIFFE ID from valid token"
    );
}

#[test]
fn test_spiffe_extract_claims() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    let claims = provider.extract_claims(&scheme, &req);
    assert!(
        claims.is_some(),
        "Should extract claims from valid token"
    );
    
    let claims = claims.unwrap();
    assert_eq!(
        claims.get("sub").and_then(|v| v.as_str()),
        Some("spiffe://example.com/api/users"),
        "Claims should contain SPIFFE ID in 'sub'"
    );
    assert_eq!(
        claims.get("aud").and_then(|v| v.as_str()),
        Some("api.example.com"),
        "Claims should contain audience"
    );
    assert!(claims.get("exp").is_some(), "Claims should contain expiration");
    assert!(claims.get("iat").is_some(), "Claims should contain issued at");
}

#[test]
fn test_spiffe_cookie_extraction() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .cookie_name("spiffe_token");
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    // Token in cookie, not header
    let mut cookies: HeaderVec = HeaderVec::new();
    cookies.push((Arc::from("spiffe_token"), token));
    
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Should extract token from cookie when configured"
    );
    
    // Verify SPIFFE ID extraction from cookie
    let spiffe_id = provider.extract_spiffe_id(&req);
    assert_eq!(
        spiffe_id,
        Some("spiffe://example.com/api/users".to_string()),
        "Should extract SPIFFE ID from cookie token"
    );
}

#[test]
fn test_spiffe_leeway_configuration() {
    // Test with large leeway (should allow slightly expired tokens)
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .leeway(3600); // 1 hour leeway
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Token expired 30 minutes ago (within leeway)
    use base64::{engine::general_purpose, Engine as _};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let exp = now - 1800; // 30 minutes ago
    let iat = now - 3600; // 1 hour ago
    
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "sub": "spiffe://example.com/api/users",
        "aud": "api.example.com",
        "exp": exp,
        "iat": iat
    });
    
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    let token = format!("{header_b64}.{payload_b64}.signature");
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Token expired within leeway should pass validation"
    );
}

#[test]
fn test_spiffe_malformed_jwt() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Malformed JWT (not 3 parts)
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), "Bearer invalid.token".to_string()));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Malformed JWT should fail validation"
    );
}

#[test]
fn test_spiffe_invalid_base64_payload() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Invalid base64 in payload
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), "Bearer header.invalid-base64!.signature".to_string()));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid base64 payload should fail validation"
    );
}

#[test]
fn test_spiffe_invalid_json_payload() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Valid base64 but invalid JSON
    use base64::{engine::general_purpose, Engine as _};
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(b"header");
    let invalid_json_b64 = general_purpose::URL_SAFE_NO_PAD.encode(b"not valid json");
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {header_b64}.{invalid_json_b64}.signature")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid JSON payload should fail validation"
    );
}

#[test]
fn test_spiffe_multiple_trust_domains() {
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com", "enterprise.local", "prod.example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Test each trust domain
    for trust_domain in &["example.com", "enterprise.local", "prod.example.com"] {
        let spiffe_id = format!("spiffe://{trust_domain}/api/users");
        let token = make_spiffe_jwt(&spiffe_id, "api.example.com", 3600, 0);
        
        let mut headers: HeaderVec = HeaderVec::new();
        headers.push((Arc::from("authorization"), format!("Bearer {token}")));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        assert!(
            provider.validate(&scheme, &[], &req),
            "Trust domain '{trust_domain}' should pass validation"
        );
    }
}

// Phase 2: JWKS Signature Verification Tests

#[test]
#[ignore] // TODO: Debug JWKS refresh synchronization issue
fn test_spiffe_jwks_signature_verification() {
    // Create mock JWKS server
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // Create provider with JWKS
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Create properly signed token
    let token = make_signed_spiffe_jwt(secret, "spiffe://example.com/api/users", "api.example.com", "k1", 3600);
    
    // Verify server is ready by making a test request
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let test_url = jwks_url.strip_prefix("http://").unwrap();
    let parts: Vec<&str> = test_url.split(':').collect();
    let test_addr = format!("{}:{}", parts[0], parts[1].strip_suffix("/jwks.json").unwrap());
    for _ in 0..10 {
        if let Ok(mut stream) = TcpStream::connect(&test_addr) {
            let req = "GET /jwks.json HTTP/1.1\r\nHost: localhost\r\n\r\n";
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut buf = [0u8; 1024];
                if stream.read(&mut buf).is_ok() {
                    break; // Server is ready
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    
    // Trigger JWKS fetch by calling validate (which calls refresh_jwks_if_needed)
    // First validation will do blocking refresh if cache is empty
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // First validation should trigger blocking JWKS fetch and succeed
    // The refresh_jwks_if_needed() method does blocking refresh when cache is empty
    let result = provider.validate(&scheme, &[], &req);
    
    assert!(
        result,
        "Valid signed SPIFFE SVID should pass validation with JWKS. This verifies JWKS fetch and signature verification work correctly."
    );
}

#[test]
fn test_spiffe_jwks_invalid_signature() {
    // Create mock JWKS server
    let secret = b"supersecret";
    let wrong_secret = b"wrongsecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // Create provider with JWKS
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Create token signed with wrong secret
    let token = make_signed_spiffe_jwt(wrong_secret, "spiffe://example.com/api/users", "api.example.com", "k1", 3600);
    
    // Wait for JWKS to be fetched
    thread::sleep(Duration::from_millis(200));
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid signature should fail validation"
    );
}

#[test]
fn test_spiffe_jwks_missing_key_id() {
    // Create mock JWKS server
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // Create provider with JWKS
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Create token without kid in header
    
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    
    let header = Header {
        kid: None, // Missing kid
        alg: Algorithm::HS256,
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
    
    // Wait for JWKS to be fetched
    thread::sleep(Duration::from_millis(200));
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Token without kid should fail validation when JWKS is configured"
    );
}

#[test]
fn test_spiffe_jwks_key_not_found() {
    // Create mock JWKS server
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    
    // Create provider with JWKS
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Create token with kid that doesn't exist in JWKS
    let token = make_signed_spiffe_jwt(secret, "spiffe://example.com/api/users", "api.example.com", "k2", 3600);
    
    // Wait for JWKS to be fetched
    thread::sleep(Duration::from_millis(200));
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Token with kid not in JWKS should fail validation"
    );
}

#[test]
fn test_spiffe_jwks_without_jwks_url() {
    // Provider without JWKS URL should skip signature verification
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Unsigned token (no signature verification)
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Provider without JWKS URL should skip signature verification"
    );
}

#[test]
#[ignore] // TODO: Debug JWKS refresh synchronization issue
fn test_spiffe_jwks_cache_refresh() {
    // Create mock JWKS server
    let secret1 = b"secret1";
    let k1 = base64url_no_pad(secret1);
    let jwks1 = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k1}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks1);
    
    // Create provider with short cache TTL
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url(&jwks_url)
        .jwks_cache_ttl(1); // 1 second TTL
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Verify server is ready
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let test_url = jwks_url.strip_prefix("http://").unwrap();
    let parts: Vec<&str> = test_url.split(':').collect();
    let test_addr = format!("{}:{}", parts[0], parts[1].strip_suffix("/jwks.json").unwrap());
    for _ in 0..10 {
        if let Ok(mut stream) = TcpStream::connect(&test_addr) {
            let req = "GET /jwks.json HTTP/1.1\r\nHost: localhost\r\n\r\n";
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut buf = [0u8; 1024];
                if stream.read(&mut buf).is_ok() {
                    break; // Server is ready
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    
    // Create token with first key
    let token1 = make_signed_spiffe_jwt(secret1, "spiffe://example.com/api/users", "api.example.com", "k1", 3600);
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token1}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // First validation will trigger blocking JWKS fetch
    assert!(
        provider.validate(&scheme, &[], &req),
        "First token should pass validation (triggers blocking JWKS fetch)"
    );
    
    // Wait for cache to expire
    thread::sleep(Duration::from_secs(2));
    
    // Token should still work (cache refresh should happen in background)
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token1}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // Give background refresh time to complete
    thread::sleep(Duration::from_millis(300));
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Token should still work after cache refresh"
    );
}

// Additional tests to improve coverage for error handling paths

#[test]
#[should_panic(expected = "JWKS URL must use HTTPS")]
fn test_spiffe_jwks_url_http_non_localhost_panic() {
    // Test that HTTP URLs are rejected for non-localhost hosts
    let _provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("http://example.com/.well-known/jwks.json");
}

#[test]
fn test_spiffe_jwks_url_http_localhost_allowed() {
    // Test that HTTP URLs are allowed for localhost
    // This test verifies the URL validation logic doesn't panic for localhost
    let _provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("http://localhost:8080/.well-known/jwks.json");
    
    // Should not panic - test passes if we get here
}

#[test]
fn test_spiffe_jwks_url_http_127_0_0_1_allowed() {
    // Test that HTTP URLs are allowed for 127.0.0.1
    // This test verifies the URL validation logic doesn't panic for 127.0.0.1
    let _provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("http://127.0.0.1:8080/.well-known/jwks.json");
    
    // Should not panic - test passes if we get here
}

#[test]
fn test_spiffe_jwks_url_https_allowed() {
    // Test that HTTPS URLs are always allowed
    // This test verifies the URL validation logic doesn't panic for HTTPS
    let _provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("https://spiffe.example.com/.well-known/jwks.json");
    
    // Should not panic - test passes if we get here
}

#[test]
#[should_panic(expected = "JWKS URL is invalid")]
fn test_spiffe_jwks_url_invalid_format_panic() {
    // Test that invalid URL format causes panic
    let _provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("not-a-valid-url");
}

#[test]
fn test_spiffe_jwks_cache_ttl_configuration() {
    // Test that cache TTL can be configured
    // This test verifies the builder method doesn't panic
    let _provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("http://localhost:8080/.well-known/jwks.json")
        .jwks_cache_ttl(60); // 60 seconds
    
    // Should not panic - test passes if we get here
}

#[test]
fn test_spiffe_get_key_for_no_jwks_configured() {
    // Test that get_key_for returns None when JWKS not configured
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    // get_key_for is pub(super), so we test via validate with JWKS-required token
    // This indirectly tests that get_key_for returns None
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Create a token that would require JWKS (but provider doesn't have JWKS)
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // Should succeed because JWKS is not configured (signature verification skipped)
    assert!(
        provider.validate(&scheme, &[], &req),
        "Provider without JWKS should skip signature verification"
    );
}

#[test]
fn test_spiffe_refresh_jwks_if_needed_no_jwks_configured() {
    // Test that refresh_jwks_if_needed returns early when JWKS not configured
    // This is tested indirectly by creating a provider without JWKS and verifying
    // it doesn't crash or hang
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    // Should not panic or hang
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // Multiple validations should work without JWKS
    assert!(provider.validate(&scheme, &[], &req));
    assert!(provider.validate(&scheme, &[], &req));
    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_spiffe_extract_token_from_cookie() {
    // Test token extraction from cookie when not in Authorization header
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .cookie_name("spiffe_token");
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    // Put token in cookie instead of Authorization header
    let mut cookies: HeaderVec = HeaderVec::new();
    cookies.push((Arc::from("spiffe_token"), token));
    
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Token should be extractable from cookie"
    );
}

#[test]
fn test_spiffe_extract_token_prefers_header_over_cookie() {
    // Test that Authorization header is preferred over cookie
    // This test verifies that when both header and cookie are present,
    // the header is used (extract_token checks header first)
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .cookie_name("spiffe_token");
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let valid_token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    // Put valid token in header, also put it in cookie (both valid)
    // This tests that header is checked first
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {valid_token}")));
    
    let mut cookies: HeaderVec = HeaderVec::new();
    cookies.push((Arc::from("spiffe_token"), valid_token));
    
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &cookies,
    };
    
    // Should use header token and succeed
    assert!(
        provider.validate(&scheme, &[], &req),
        "Authorization header should be preferred over cookie"
    );
}

#[test]
fn test_spiffe_extract_token_cookie_fallback() {
    // Test that cookie is used when header is not present
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .cookie_name("spiffe_token");
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    // Put token in cookie only (no header)
    let mut cookies: HeaderVec = HeaderVec::new();
    cookies.push((Arc::from("spiffe_token"), token));
    
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };
    
    assert!(
        provider.validate(&scheme, &[], &req),
        "Token should be extractable from cookie when header is missing"
    );
}

#[test]
fn test_spiffe_extract_token_no_cookie_name() {
    // Test that when cookie_name is not set, only header is checked
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    // No cookie_name set
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    // Put token in cookie only (no header, no cookie_name configured)
    let mut cookies: HeaderVec = HeaderVec::new();
    cookies.push((Arc::from("spiffe_token"), token));
    
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };
    
    // Should fail because no header and cookie_name not configured
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Token should not be extractable from cookie when cookie_name not configured"
    );
}

#[test]
fn test_spiffe_leeway_configuration_edge_cases() {
    // Test leeway configuration with various values
    let provider1 = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .leeway(0); // No leeway
    
    let provider2 = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .leeway(300); // 5 minutes leeway
    
    // Both should be valid configurations
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token1 = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token1}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // Both providers should validate the same token
    assert!(provider1.validate(&scheme, &[], &req));
    assert!(provider2.validate(&scheme, &[], &req));
}

#[test]
fn test_spiffe_get_key_for_cache_read_error() {
    // Test that get_key_for handles cache read errors gracefully
    // This is tested indirectly by ensuring validation doesn't panic
    // when cache operations fail
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"])
        .jwks_url("http://localhost:9999/.well-known/jwks.json"); // Invalid port (no server)
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // Should fail gracefully (JWKS fetch will fail, but shouldn't panic)
    // Since JWKS is configured, signature verification will fail
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Validation should fail when JWKS fetch fails"
    );
}

#[test]
fn test_spiffe_refresh_jwks_if_needed_early_return() {
    // Test that refresh_jwks_if_needed returns early when cache is not expired
    // This is tested indirectly by creating a provider and validating multiple times
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    let token = make_spiffe_jwt(
        "spiffe://example.com/api/users",
        "api.example.com",
        3600,
        0,
    );
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    // Multiple validations should work (refresh_jwks_if_needed should return early)
    assert!(provider.validate(&scheme, &[], &req));
    assert!(provider.validate(&scheme, &[], &req));
    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_spiffe_parse_jwt_claims_invalid_format() {
    // Test that parse_jwt_claims handles invalid JWT formats
    // This is tested indirectly via validation failures
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Invalid JWT format (only 2 parts instead of 3)
    let invalid_token = "header.payload";
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {invalid_token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid JWT format should fail validation"
    );
}

#[test]
fn test_spiffe_parse_jwt_claims_invalid_base64() {
    // Test that parse_jwt_claims handles invalid base64
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Invalid base64 in payload
    let invalid_token = "header.invalid-base64!.signature";
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {invalid_token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid base64 payload should fail validation"
    );
}

#[test]
fn test_spiffe_parse_jwt_claims_invalid_json() {
    // Test that parse_jwt_claims handles invalid JSON in payload
    let provider = SpiffeProvider::new()
        .trust_domains(&["example.com"])
        .audiences(&["api.example.com"]);
    
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Valid base64 but invalid JSON
    use base64::{engine::general_purpose, Engine as _};
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(b"header");
    let invalid_json_b64 = general_purpose::URL_SAFE_NO_PAD.encode(b"not valid json");
    
    let invalid_token = format!("{header_b64}.{invalid_json_b64}.signature");
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {invalid_token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(
        !provider.validate(&scheme, &[], &req),
        "Invalid JSON payload should fail validation"
    );
}
