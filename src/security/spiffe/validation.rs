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
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use super::SpiffeProvider;

/// SPIFFE ID format regex: `spiffe://trust-domain/path`
/// Trust domain: alphanumeric, dots, hyphens, underscores
/// Path: required, must start with `/` (root path `/` is valid)
/// Per SPIFFE specification, the path component is mandatory
use once_cell::sync::Lazy;

static SPIFFE_ID_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^spiffe://([a-zA-Z0-9._-]+)/.*$")
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

    // Parse JWT claims first (needed for SPIFFE ID validation)
    // We'll verify signature after basic validation if JWKS is configured
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

    // JWT signature verification (if JWKS URL configured)
    // Done after basic validation to ensure we have valid SPIFFE ID first
    if provider.jwks_url.is_some()
        && !verify_signature(token, provider) {
            warn!("SPIFFE SVID signature verification failed");
            return false;
        }

    debug!(
        "SPIFFE SVID validated successfully: spiffe_id={}, trust_domain={}",
        spiffe_id, trust_domain
    );
    true
}

/// Verify JWT signature using JWKS.
///
/// Parses the JWT header to get the key ID (kid), fetches the decoding key
/// from JWKS cache, and verifies the signature.
fn verify_signature(token: &str, provider: &SpiffeProvider) -> bool {
    // Parse JWT header to get kid
    let header = match jsonwebtoken::decode_header(token) {
        Ok(h) => h,
        Err(e) => {
            debug!("Failed to parse JWT header: {:?}", e);
            return false;
        }
    };
    
    let kid = match header.kid {
        Some(k) => k,
        None => {
            debug!("JWT header missing 'kid' (key ID)");
            return false;
        }
    };
    
    // Get decoding key from JWKS cache
    let decoding_key = match provider.get_key_for(&kid) {
        Some(k) => k,
        None => {
            debug!("Key '{}' not found in JWKS cache", kid);
            return false;
        }
    };
    
    // Verify signature using jsonwebtoken
    // We only decode to verify signature, we don't need the claims (already parsed)
    // Disable expiration check since we already validated it above
    let mut validation = jsonwebtoken::Validation::new(header.alg);
    validation.validate_exp = false; // Already validated above
    // Signature validation is enabled by default in jsonwebtoken
    
    match jsonwebtoken::decode::<serde_json::Value>(token, &decoding_key, &validation) {
        Ok(_) => {
            debug!("SPIFFE SVID signature verified successfully");
            true
        }
        Err(e) => {
            debug!("SPIFFE SVID signature verification failed: {:?}", e);
            false
        }
    }
}

/// Refresh JWKS keys from URL.
///
/// This is a simplified version of JwksBearerProvider's refresh logic,
/// adapted for SPIFFE provider.
pub(super) fn refresh_jwks_internal(
    cache: &Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
    jwks_url: &str,
    refresh_in_progress: &Arc<AtomicBool>,
    refresh_complete: &Arc<(Mutex<()>, Condvar)>,
    already_claimed: bool,
) {
    if !already_claimed
        && refresh_in_progress
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return; // Another thread is refreshing
        }
    
    let refresh_start = Instant::now();
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            refresh_in_progress.store(false, Ordering::Release);
            let (lock, cvar) = &**refresh_complete;
            let _guard = lock.lock().unwrap();
            cvar.notify_all();
            return;
        }
    };
    
        let mut body_opt: Option<String> = None;
        for attempt in 0..2 {
            match client.get(jwks_url).send() {
                Ok(r) => {
                    if r.status().is_success() {
                        if let Ok(t) = r.text() {
                            body_opt = Some(t);
                            break;
                        } else {
                            debug!("SPIFFE JWKS fetch attempt {}: failed to read response body", attempt + 1);
                        }
                    } else {
                        debug!("SPIFFE JWKS fetch attempt {}: HTTP status {}", attempt + 1, r.status());
                    }
                }
                Err(e) => {
                    debug!("SPIFFE JWKS fetch attempt {}: request failed: {:?}", attempt + 1, e);
                }
            }
        }
    
    let body = match body_opt {
        Some(b) => b,
        None => {
            refresh_in_progress.store(false, Ordering::Release);
            let (lock, cvar) = &**refresh_complete;
            let _guard = lock.lock().unwrap();
            cvar.notify_all();
            return;
        }
    };
    
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            refresh_in_progress.store(false, Ordering::Release);
            let (lock, cvar) = &**refresh_complete;
            let _guard = lock.lock().unwrap();
            cvar.notify_all();
            return;
        }
    };
    
    let mut new_map: HashMap<String, jsonwebtoken::DecodingKey> = HashMap::new();
    if let Some(keys) = parsed.get("keys").and_then(|v| v.as_array()) {
        for k in keys {
            let kid = k.get("kid").and_then(|v| v.as_str()).unwrap_or("");
            let kty = k.get("kty").and_then(|v| v.as_str()).unwrap_or("");
            let alg = k.get("alg").and_then(|v| v.as_str()).unwrap_or("");
            
            // HMAC (oct) keys for HS* algorithms
            if kty.eq_ignore_ascii_case("oct")
                && (alg.eq_ignore_ascii_case("HS256")
                    || alg.eq_ignore_ascii_case("HS384")
                    || alg.eq_ignore_ascii_case("HS512"))
            {
                if let Some(kval) = k.get("k").and_then(|v| v.as_str()) {
                    use base64::Engine as _;
                    if let Ok(secret) =
                        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(kval)
                    {
                        let dk = jsonwebtoken::DecodingKey::from_secret(&secret);
                        new_map.insert(kid.to_string(), dk);
                    }
                }
                continue;
            }
            
            // RSA public keys for RS* algorithms
            if kty.eq_ignore_ascii_case("RSA")
                && (alg.eq_ignore_ascii_case("RS256")
                    || alg.eq_ignore_ascii_case("RS384")
                    || alg.eq_ignore_ascii_case("RS512"))
            {
                let n = match k.get("n").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => continue,
                };
                let e = match k.get("e").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => continue,
                };
                if let Ok(dk) = jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                    new_map.insert(kid.to_string(), dk);
                }
                continue;
            }
        }
    }
    
    let key_count = new_map.len();
    let refresh_duration = refresh_start.elapsed();
    
    if let Ok(mut guard) = cache.write() {
        *guard = (Instant::now(), new_map);
    }
    
    refresh_in_progress.store(false, Ordering::Release);
    
    let (lock, cvar) = &**refresh_complete;
    let _guard = lock.lock().unwrap();
    cvar.notify_all();
    
    debug!(
        "SPIFFE JWKS refresh completed in {:?} (keys: {})",
        refresh_duration, key_count
    );
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
        // Valid SPIFFE IDs (with required path component)
        assert!(is_valid_spiffe_id("spiffe://example.com/api/users"));
        assert!(is_valid_spiffe_id("spiffe://enterprise.local/windows/service"));
        assert!(is_valid_spiffe_id("spiffe://prod.example.com/frontend/web"));
        assert!(is_valid_spiffe_id("spiffe://example.com/")); // Root path is valid
        assert!(is_valid_spiffe_id("spiffe://example.com/path"));
        
        // Invalid SPIFFE IDs
        assert!(!is_valid_spiffe_id("invalid"));
        assert!(!is_valid_spiffe_id("http://example.com"));
        assert!(!is_valid_spiffe_id("spiffe://"));
        // Path component is required per SPIFFE specification
        assert!(!is_valid_spiffe_id("spiffe://example.com"));
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

