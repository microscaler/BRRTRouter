//! Comprehensive tests for SPIFFE security provider
//!
//! Tests SPIFFE JWT SVID validation including:
//! - SPIFFE ID format validation
//! - Trust domain whitelist enforcement
//! - Audience validation
//! - Expiration checking
//! - JWT claim extraction
//! - Integration with SecurityProvider trait

use brrtrouter::security::{SecurityProvider, SecurityRequest, SpiffeProvider};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{dispatcher::HeaderVec, router::ParamVec};
use serde_json::json;
use std::sync::Arc;

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
    format!("{}.{}.signature", header_b64, payload_b64)
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
    
    format!("{}.{}.signature", header_b64, payload_b64)
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
    
    format!("{}.{}.signature", header_b64, payload_b64)
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
        headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        // Should pass validation (format is valid)
        assert!(
            provider.validate(&scheme, &[], &req),
            "Valid SPIFFE ID '{}' should pass validation",
            spiffe_id
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
        let token = format!("{}.{}.signature", header_b64, payload_b64);
        
        let mut headers: HeaderVec = HeaderVec::new();
        headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        // Should fail validation (format is invalid)
        assert!(
            !provider.validate(&scheme, &[], &req),
            "Invalid SPIFFE ID '{}' should fail validation",
            spiffe_id
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
        headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    
    let mut headers: HeaderVec = HeaderVec::new();
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
    let token = format!("{}.{}.signature", header_b64, payload_b64);
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    let token = format!("{}.{}.signature", header_b64, payload_b64);
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    let token = format!("{}.{}.signature", header_b64, payload_b64);
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    cookies.push((Arc::from("spiffe_token"), token.clone()));
    
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
    let token = format!("{}.{}.signature", header_b64, payload_b64);
    
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
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
    headers.push((Arc::from("authorization"), format!("Bearer {}.{}.signature", header_b64, invalid_json_b64)));
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
        let spiffe_id = format!("spiffe://{}/api/users", trust_domain);
        let token = make_spiffe_jwt(&spiffe_id, "api.example.com", 3600, 0);
        
        let mut headers: HeaderVec = HeaderVec::new();
        headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
        let req = SecurityRequest {
            headers: &headers,
            query: &ParamVec::new(),
            cookies: &HeaderVec::new(),
        };
        
        assert!(
            provider.validate(&scheme, &[], &req),
            "Trust domain '{}' should pass validation",
            trust_domain
        );
    }
}

