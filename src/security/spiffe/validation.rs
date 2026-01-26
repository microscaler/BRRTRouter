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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use super::SpiffeProvider;

/// SPIFFE ID format regex: `spiffe://trust-domain/path`
/// Trust domain: alphanumeric, dots, hyphens, underscores
/// Path: required, must start with `/` (root path `/` is valid)
/// Per SPIFFE specification, the path component is mandatory
use once_cell::sync::Lazy;

static SPIFFE_ID_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^spiffe://([a-zA-Z0-9._-]+)/.*$").expect("SPIFFE ID regex should be valid")
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
            warn!(
                "SPIFFE SVID has invalid trust domain in SPIFFE ID: {}",
                spiffe_id
            );
            return false;
        }
    };

    // Validate trust domain against whitelist
    // Security: Fail-secure - reject if trust domains not configured
    if provider.trust_domains.is_empty() {
        warn!(
            "SPIFFE SVID validation failed: trust domains not configured (rejecting for security)"
        );
        return false;
    }

    if !provider.trust_domains.contains(&trust_domain) {
        warn!(
            "SPIFFE SVID trust domain '{}' not in whitelist",
            trust_domain
        );
        return false;
    }

    // Validate audience
    // Security: Fail-secure - reject if audiences not configured
    if provider.audiences.is_empty() {
        warn!("SPIFFE SVID validation failed: audiences not configured (rejecting for security)");
        return false;
    }

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

    // Validate expiration
    if !validate_expiration(&claims, provider.leeway_secs) {
        warn!("SPIFFE SVID expired or invalid expiration claim");
        return false;
    }

    // Validate issuer (iss) claim if present
    // Per SPIFFE spec: if iss is present, it should match the trust domain from sub
    if let Some(iss) = claims.get("iss").and_then(|v| v.as_str()) {
        if iss != trust_domain {
            warn!(
                "SPIFFE SVID issuer '{}' does not match trust domain '{}' from sub claim",
                iss, trust_domain
            );
            return false;
        }
    }

    // Validate issued at (iat) claim if present
    // Per SPIFFE spec: iat should be present and not in the future
    if let Some(iat) = claims.get("iat").and_then(|v| v.as_i64()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Allow leeway for clock skew (token issued slightly in future due to clock differences)
        // Use saturating_add to prevent integer overflow (security: overflow could bypass validation)
        let max_allowed_iat = now.saturating_add(provider.leeway_secs as i64);
        if iat > max_allowed_iat {
            warn!(
                "SPIFFE SVID issued at (iat={}) is too far in the future (now={}, leeway={})",
                iat, now, provider.leeway_secs
            );
            return false;
        }
    }

    // Validate not before (nbf) claim if present
    // Per JWT spec: token should not be used before nbf time
    if let Some(nbf) = claims.get("nbf").and_then(|v| v.as_i64()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Allow leeway for clock skew (token not yet valid due to clock differences)
        // Use saturating_sub to prevent integer overflow (security: overflow could bypass validation)
        let min_allowed_time = nbf.saturating_sub(provider.leeway_secs as i64);
        if now < min_allowed_time {
            warn!(
                "SPIFFE SVID not yet valid (nbf={}, now={}, leeway={})",
                nbf, now, provider.leeway_secs
            );
            return false;
        }
    }

    // Token revocation checking (if revocation checker configured)
    // Check jti against revocation list before signature verification
    if let Some(revocation_checker) = &provider.revocation_checker {
        if let Some(jti) = claims.get("jti").and_then(|v| v.as_str()) {
            if revocation_checker.is_revoked(jti) {
                warn!("SPIFFE SVID token revoked: jti={}", jti);
                return false;
            }
        }
    }

    // JWT signature verification is REQUIRED for security
    // Without signature verification, attackers can forge tokens with arbitrary claims
    // Security: Fail-secure - reject if JWKS URL not configured
    if provider.jwks_url.is_none() {
        warn!(
            "SPIFFE SVID validation failed: JWKS URL not configured. Signature verification is required for security. Configure with `.jwks_url()`"
        );
        return false;
    }

    // Verify signature using JWKS
    // Done after basic validation to ensure we have valid SPIFFE ID first
    if !verify_signature(token, provider) {
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

    // Get decoding key and algorithm from JWKS cache
    let (decoding_key, jwks_alg) = match provider.get_key_for(&kid) {
        Some((k, alg)) => (k, alg),
        None => {
            debug!("Key '{}' not found in JWKS cache", kid);
            return false;
        }
    };

    // Security: Validate that token algorithm matches JWKS key algorithm
    // This prevents algorithm confusion attacks where an attacker uses a different algorithm
    // than the one specified in the JWKS key
    if header.alg != jwks_alg {
        warn!(
            "SPIFFE SVID algorithm mismatch: token uses {:?}, but JWKS key '{}' specifies {:?}. This prevents algorithm confusion attacks.",
            header.alg, kid, jwks_alg
        );
        return false;
    }

    // Verify signature using jsonwebtoken
    // We only decode to verify signature, we don't need the claims (already parsed)
    // Disable expiration and audience checks since we already validated them above
    let mut validation = jsonwebtoken::Validation::new(header.alg);
    validation.validate_exp = false; // Already validated above
    validation.validate_aud = false; // Already validated above (SPIFFE audience validation)
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
    cache: &Arc<
        RwLock<(
            Instant,
            HashMap<String, (jsonwebtoken::DecodingKey, jsonwebtoken::Algorithm)>,
        )>,
    >,
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
                        debug!(
                            "SPIFFE JWKS fetch attempt {}: failed to read response body",
                            attempt + 1
                        );
                    }
                } else {
                    debug!(
                        "SPIFFE JWKS fetch attempt {}: HTTP status {}",
                        attempt + 1,
                        r.status()
                    );
                }
            }
            Err(e) => {
                debug!(
                    "SPIFFE JWKS fetch attempt {}: request failed: {:?}",
                    attempt + 1,
                    e
                );
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

    let mut new_map: HashMap<String, (jsonwebtoken::DecodingKey, jsonwebtoken::Algorithm)> =
        HashMap::new();
    if let Some(keys) = parsed.get("keys").and_then(|v| v.as_array()) {
        for k in keys {
            let kid = k.get("kid").and_then(|v| v.as_str()).unwrap_or("");
            let kty = k.get("kty").and_then(|v| v.as_str()).unwrap_or("");
            let alg = k.get("alg").and_then(|v| v.as_str()).unwrap_or("");

            // HMAC (oct) keys for HS* algorithms
            // Note: HMAC keys are symmetric and not typically used in production JWKS (security risk)
            // However, we support them for testing purposes
            if kty.eq_ignore_ascii_case("oct")
                && (alg.eq_ignore_ascii_case("HS256")
                    || alg.eq_ignore_ascii_case("HS384")
                    || alg.eq_ignore_ascii_case("HS512"))
            {
                let k = match k.get("k").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => {
                        debug!("HMAC key missing 'k' parameter, skipping");
                        continue;
                    }
                };
                // Decode base64url-encoded key
                use base64::Engine as _;
                let key_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(k) {
                    Ok(b) => b,
                    Err(e) => {
                        debug!("Failed to decode HMAC key 'k' parameter: {:?}", e);
                        continue;
                    }
                };
                // Create decoding key from secret bytes
                let dk = jsonwebtoken::DecodingKey::from_secret(&key_bytes);
                // Parse algorithm from JWKS key
                let jwks_algorithm = if alg.eq_ignore_ascii_case("HS256") {
                    jsonwebtoken::Algorithm::HS256
                } else if alg.eq_ignore_ascii_case("HS384") {
                    jsonwebtoken::Algorithm::HS384
                } else if alg.eq_ignore_ascii_case("HS512") {
                    jsonwebtoken::Algorithm::HS512
                } else {
                    debug!("Unsupported HMAC algorithm in JWKS: {}, skipping", alg);
                    continue;
                };
                new_map.insert(kid.to_string(), (dk, jwks_algorithm));
                debug!("Added HMAC key to JWKS cache: kid={}, alg={}", kid, alg);
                continue;
            }

            // ECDSA (EC) keys for ES* algorithms
            // JWKS format: EC keys use "x" and "y" coordinates (base64url-encoded)
            // Note: jsonwebtoken only supports ES256 and ES384, not ES512
            if kty.eq_ignore_ascii_case("EC")
                && (alg.eq_ignore_ascii_case("ES256") || alg.eq_ignore_ascii_case("ES384"))
            {
                let x = match k.get("x").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => {
                        debug!("ECDSA key missing 'x' coordinate, skipping");
                        continue;
                    }
                };
                let y = match k.get("y").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => {
                        debug!("ECDSA key missing 'y' coordinate, skipping");
                        continue;
                    }
                };
                // Parse algorithm from JWKS key
                // Note: jsonwebtoken only supports ES256 and ES384, not ES512
                let jwks_algorithm = if alg.eq_ignore_ascii_case("ES256") {
                    jsonwebtoken::Algorithm::ES256
                } else if alg.eq_ignore_ascii_case("ES384") {
                    jsonwebtoken::Algorithm::ES384
                } else {
                    debug!("Unsupported ECDSA algorithm in JWKS: {} (only ES256 and ES384 are supported), skipping", alg);
                    continue;
                };
                // jsonwebtoken crate supports from_ec_components for ECDSA keys
                if let Ok(dk) = jsonwebtoken::DecodingKey::from_ec_components(x, y) {
                    new_map.insert(kid.to_string(), (dk, jwks_algorithm));
                    debug!("Added ECDSA key to JWKS cache: kid={}, alg={}", kid, alg);
                } else {
                    debug!("Failed to parse ECDSA key: kid={}, alg={}", kid, alg);
                }
                continue;
            }

            // RSA public keys for RS* algorithms
            if kty.eq_ignore_ascii_case("RSA")
                && (alg.eq_ignore_ascii_case("RS256")
                    || alg.eq_ignore_ascii_case("RS384")
                    || alg.eq_ignore_ascii_case("RS512"))
            {
                // Parse algorithm from JWKS key
                let jwks_algorithm = if alg.eq_ignore_ascii_case("RS256") {
                    jsonwebtoken::Algorithm::RS256
                } else if alg.eq_ignore_ascii_case("RS384") {
                    jsonwebtoken::Algorithm::RS384
                } else if alg.eq_ignore_ascii_case("RS512") {
                    jsonwebtoken::Algorithm::RS512
                } else {
                    debug!("Unsupported RSA algorithm in JWKS: {}, skipping", alg);
                    continue;
                };
                let n = match k.get("n").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => continue,
                };
                let e = match k.get("e").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => continue,
                };
                if let Ok(dk) = jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                    new_map.insert(kid.to_string(), (dk, jwks_algorithm));
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

    // Validate trust domain against whitelist
    // Security: Fail-secure - reject if trust domains not configured
    if provider.trust_domains.is_empty() {
        return None; // Trust domains must be configured
    }

    let trust_domain = extract_trust_domain(&spiffe_id)?;
    if !provider.trust_domains.contains(&trust_domain) {
        return None;
    }

    Some(spiffe_id)
}

/// Extract claims from a token.
///
/// This is a helper for `SpiffeProvider::extract_claims()`.
pub(super) fn extract_claims_from_token(token: &str, _provider: &SpiffeProvider) -> Option<Value> {
    parse_jwt_claims(token).ok()
}

/// Extract JWT ID (jti) from a token.
///
/// The `jti` claim provides a unique identifier for the token, essential for:
/// - Token revocation (blacklisting)
/// - Audit logging
/// - Replay prevention
/// - Security incident response
///
/// This is a helper for `SpiffeProvider::extract_jti()`.
pub(super) fn extract_jti_from_token(token: &str) -> Option<String> {
    let claims = parse_jwt_claims(token).ok()?;
    claims.get("jti")?.as_str().map(|s| s.to_string())
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
    // Check for overflow explicitly (security: overflow could bypass expiration)
    let expiration_time = match exp.checked_add(leeway_secs as i64) {
        Some(t) => t,
        None => {
            // Overflow occurred - fall back to checking expiration without leeway
            // This is safe: if token is expired without leeway, reject it
            // If token is not expired, accept it (can't apply leeway due to overflow, but token is valid)
            debug!(
                "SPIFFE SVID expiration calculation overflow: exp={}, leeway={}, checking without leeway",
                exp, leeway_secs
            );
            // Check expiration without leeway (security: expired tokens still rejected)
            if now > exp {
                debug!(
                    "SPIFFE SVID expired (overflow prevented leeway application): exp={}, now={}",
                    exp, now
                );
                return false;
            }
            // Token not expired, accept it (overflow just means we can't apply leeway)
            return true;
        }
    };

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
        assert!(is_valid_spiffe_id(
            "spiffe://enterprise.local/windows/service"
        ));
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
