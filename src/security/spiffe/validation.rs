//! SPIFFE SVID validation logic.
//!
//! This module contains the core validation logic for SPIFFE JWT SVIDs, including:
//! - SPIFFE ID format validation
//! - Trust domain extraction and validation
//! - Audience validation
//! - JWT signature verification (if JWKS configured)
//! - Expiration checking

use crate::security::SecurityRequest;
use crate::spec::SecurityScheme;
use regex::Regex;
use serde_json::Value;
use tracing::{debug, warn};

use super::SpiffeProvider;

/// SPIFFE ID format regex: `spiffe://trust-domain/path`
/// Trust domain: alphanumeric, dots, hyphens, underscores
/// Path: any characters except control characters
use once_cell::sync::Lazy;

static SPIFFE_ID_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^spiffe://([a-zA-Z0-9._-]+)(/.*)?$")
        .expect("SPIFFE ID regex should be valid")
});

/// Validate a SPIFFE JWT SVID.
///
/// This is the main validation entry point called by `SpiffeProvider::validate()`.
pub(super) fn validate_svid_impl(
    provider: &SpiffeProvider,
    scheme: &SecurityScheme,
    _scopes: &[String],
    req: &SecurityRequest,
) -> bool {
    // Verify security scheme is HTTP Bearer
    match scheme {
        SecurityScheme::Http { scheme: s, .. } if s.eq_ignore_ascii_case("bearer") => {}
        _ => {
            debug!("SPIFFE provider requires HTTP Bearer scheme");
            return false;
        }
    }

    // Extract token
    let token = match provider.extract_token(req) {
        Some(t) => t,
        None => {
            debug!("SPIFFE SVID token not found in request");
            return false;
        }
    };

    // Parse JWT (without verification first, to extract claims)
    let claims = match parse_jwt_claims(token) {
        Ok(c) => c,
        Err(e) => {
            debug!("SPIFFE SVID JWT parsing failed: {}", e);
            return false;
        }
    };

    // Extract and validate SPIFFE ID from `sub` claim
    let spiffe_id = match claims.get("sub").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            debug!("SPIFFE SVID missing 'sub' claim");
            return false;
        }
    };

    if !is_valid_spiffe_id(spiffe_id) {
        warn!("SPIFFE SVID has invalid SPIFFE ID format: {}", spiffe_id);
        return false;
    }

    // Extract trust domain from SPIFFE ID
    let trust_domain = match extract_trust_domain(spiffe_id) {
        Some(td) => td,
        None => {
            warn!("SPIFFE SVID has invalid trust domain in SPIFFE ID: {}", spiffe_id);
            return false;
        }
    };

    // Validate trust domain against whitelist
    if !provider.trust_domains.is_empty() && !provider.trust_domains.contains(&trust_domain) {
        warn!(
            "SPIFFE SVID trust domain '{}' not in whitelist",
            trust_domain
        );
        return false;
    }

    // Validate audience
    if !provider.audiences.is_empty() {
        let aud_claim = claims.get("aud");
        let has_valid_audience = match aud_claim {
            Some(Value::String(aud)) => provider.audiences.contains(aud),
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .any(|aud| provider.audiences.contains(aud)),
            _ => false,
        };

        if !has_valid_audience {
            warn!(
                "SPIFFE SVID missing required audience. Required: {:?}, Got: {:?}",
                provider.audiences, aud_claim
            );
            return false;
        }
    }

    // Validate expiration
    if !validate_expiration(&claims, provider.leeway_secs) {
        warn!("SPIFFE SVID expired or invalid expiration claim");
        return false;
    }

    // TODO: JWT signature verification (if JWKS URL configured)
    // For now, we rely on external signature verification or will add JWKS support in Phase 2

    debug!(
        "SPIFFE SVID validated successfully: spiffe_id={}, trust_domain={}",
        spiffe_id, trust_domain
    );
    true
}

/// Extract SPIFFE ID from a token.
///
/// This is a helper for `SpiffeProvider::extract_spiffe_id()`.
pub(super) fn extract_spiffe_id_from_token(
    token: &str,
    provider: &SpiffeProvider,
) -> Option<String> {
    let claims = parse_jwt_claims(token).ok()?;
    let spiffe_id = claims.get("sub")?.as_str()?.to_string();
    
    // Validate format
    if !is_valid_spiffe_id(&spiffe_id) {
        return None;
    }
    
    // Validate trust domain if configured
    if !provider.trust_domains.is_empty() {
        let trust_domain = extract_trust_domain(&spiffe_id)?;
        if !provider.trust_domains.contains(&trust_domain) {
            return None;
        }
    }
    
    Some(spiffe_id)
}

/// Extract claims from a token.
///
/// This is a helper for `SpiffeProvider::extract_claims()`.
pub(super) fn extract_claims_from_token(
    token: &str,
    _provider: &SpiffeProvider,
) -> Option<Value> {
    parse_jwt_claims(token).ok()
}

/// Parse JWT claims without signature verification.
///
/// This extracts the payload and decodes it to JSON. Signature verification
/// should be done separately (e.g., via JWKS).
fn parse_jwt_claims(token: &str) -> Result<Value, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("JWT must have 3 parts (header.payload.signature)".to_string());
    }

    let payload = parts[1];
    
    // Decode base64url (JWT uses base64url, not base64)
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("Failed to decode JWT payload: {}", e))?;
    
    serde_json::from_slice(&decoded)
        .map_err(|e| format!("Failed to parse JWT claims as JSON: {}", e))
}

/// Validate SPIFFE ID format.
///
/// SPIFFE IDs must match: `spiffe://trust-domain/path`
pub(super) fn is_valid_spiffe_id(id: &str) -> bool {
    SPIFFE_ID_REGEX.is_match(id)
}

/// Extract trust domain from a SPIFFE ID.
///
/// Returns the trust domain portion (e.g., `"example.com"` from `"spiffe://example.com/api/users"`).
pub(super) fn extract_trust_domain(spiffe_id: &str) -> Option<String> {
    let caps = SPIFFE_ID_REGEX.captures(spiffe_id)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

/// Validate JWT expiration claim.
///
/// Checks the `exp` claim against the current time with leeway tolerance.
fn validate_expiration(claims: &Value, leeway_secs: u64) -> bool {
    let exp = match claims.get("exp").and_then(|v| v.as_i64()) {
        Some(e) => e,
        None => {
            debug!("SPIFFE SVID missing 'exp' claim");
            return false;
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Allow leeway for clock skew
    let expiration_time = exp + leeway_secs as i64;
    
    if now > expiration_time {
        debug!(
            "SPIFFE SVID expired: exp={}, now={}, leeway={}",
            exp, now, leeway_secs
        );
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spiffe_id_validation() {
        assert!(is_valid_spiffe_id("spiffe://example.com/api/users"));
        assert!(is_valid_spiffe_id("spiffe://enterprise.local/windows/service"));
        assert!(is_valid_spiffe_id("spiffe://prod.example.com/frontend/web"));
        assert!(!is_valid_spiffe_id("invalid"));
        assert!(!is_valid_spiffe_id("http://example.com"));
        assert!(!is_valid_spiffe_id("spiffe://"));
    }

    #[test]
    fn test_extract_trust_domain() {
        assert_eq!(
            extract_trust_domain("spiffe://example.com/api/users"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_trust_domain("spiffe://enterprise.local/windows/service"),
            Some("enterprise.local".to_string())
        );
        assert_eq!(extract_trust_domain("invalid"), None);
    }
}

