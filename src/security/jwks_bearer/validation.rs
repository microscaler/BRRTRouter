//! JWT validation and claims extraction logic
//!
//! This module contains the core validation logic for JWKS-based JWT validation,
//! including token validation, claims extraction, and cache management.
//!
//! Story 9.6: Structured JWT logging is integrated at key decision points.
//! All JWT fields (issuer, subject, client_id, session_id, jti, token_version,
//! actor_subject) are extracted from claims and logged with appropriate log levels.

use crate::security::jwks_bearer::{DecisionSource, JwtLogFields, JwtTokenStatus};
use crate::security::SecurityRequest;
use crate::spec::SecurityScheme;
use jsonwebtoken;
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{debug, warn};

/// Internal error types for JWT validation
///
/// These error types provide detailed information about validation failures
/// for better observability and debugging. They are used internally and
/// logged via structured logging, but the public API still returns `bool`
/// to maintain backward compatibility.
#[derive(Debug, Clone)]
pub(super) enum ValidationError {
    /// Token is missing from request (no Authorization header or cookie)
    MissingToken,
    /// Token format is invalid (cannot parse header)
    InvalidTokenFormat { error: String },
    /// Token header is missing required 'kid' (key ID)
    MissingKeyId,
    /// Key not found in JWKS for the given kid
    MissingKey { kid: String },
    /// Token signature is invalid
    InvalidSignature,
    /// Token has expired
    ExpiredToken { exp: i64, now: i64 },
    /// Token issuer doesn't match expected value
    InvalidIssuer {
        expected: Option<String>,
        got: Option<String>,
    },
    /// Token audience doesn't match expected value
    InvalidAudience {
        expected: Option<String>,
        got: Option<String>,
    },
    /// Token is missing a required claim
    MissingRequiredClaim { claim: String },
    /// Token uses an unsupported algorithm
    UnsupportedAlgorithm { alg: String },
    /// Token has wrong or missing typ claim (RFC 9068)
    InvalidTokenType {
        expected: String,
        got: Option<String>,
    },
    /// JWKS fetch failed
    #[allow(dead_code)] // Reserved for future use when JWKS fetch errors are tracked
    JwksFetchError { url: String, error: String },
    /// Token is missing required scopes
    InsufficientScopes {
        required: Vec<String>,
        got: Vec<String>,
    },
    /// Token was explicitly revoked by its identity provider.
    TokenRevoked,
    /// Token version is stale.
    StaleToken,
    /// Authoritative token-status dependency is unavailable.
    TokenStatusUnavailable,
    /// Required token-status claims are missing or malformed.
    InvalidTokenStatusClaims,
    /// Security scheme doesn't match (not HTTP Bearer)
    InvalidSecurityScheme { scheme: String },
}

impl ValidationError {
    /// Get a human-readable error message
    #[allow(dead_code)] // Reserved for future use (e.g., error callbacks)
    fn message(&self) -> &'static str {
        match self {
            ValidationError::MissingToken => "missing token",
            ValidationError::InvalidTokenFormat { .. } => "invalid token format",
            ValidationError::MissingKeyId => "missing key ID",
            ValidationError::MissingKey { .. } => "key not found in JWKS",
            ValidationError::InvalidSignature => "invalid signature",
            ValidationError::ExpiredToken { .. } => "token expired",
            ValidationError::InvalidIssuer { .. } => "invalid issuer",
            ValidationError::InvalidAudience { .. } => "invalid audience",
            ValidationError::MissingRequiredClaim { .. } => "missing required claim",
            ValidationError::UnsupportedAlgorithm { .. } => "unsupported algorithm",
            ValidationError::InvalidTokenType { .. } => "invalid token type",
            ValidationError::JwksFetchError { .. } => "JWKS fetch failed",
            ValidationError::InsufficientScopes { .. } => "insufficient scopes",
            ValidationError::TokenRevoked => "token revoked",
            ValidationError::StaleToken => "stale token",
            ValidationError::TokenStatusUnavailable => "token status unavailable",
            ValidationError::InvalidTokenStatusClaims => "invalid token status claims",
            ValidationError::InvalidSecurityScheme { .. } => "invalid security scheme",
        }
    }

    /// Get a structured error reason for logging
    fn error_reason(&self) -> String {
        match self {
            ValidationError::MissingToken => "missing_token".to_string(),
            ValidationError::InvalidTokenFormat { error } => {
                format!("invalid_token_format: {}", error)
            }
            ValidationError::MissingKeyId => "missing_kid".to_string(),
            ValidationError::MissingKey { kid } => format!("key_not_found: {}", kid),
            ValidationError::InvalidSignature => "invalid_signature".to_string(),
            ValidationError::ExpiredToken { exp: _, now: _ } => "token_expired".to_string(),
            ValidationError::InvalidIssuer {
                expected: _,
                got: _,
            } => "invalid_issuer".to_string(),
            ValidationError::InvalidAudience {
                expected: _,
                got: _,
            } => "invalid_audience".to_string(),
            ValidationError::MissingRequiredClaim { claim } => {
                format!("missing_required_claim: {}", claim)
            }
            ValidationError::UnsupportedAlgorithm { alg } => {
                format!("unsupported_algorithm: {}", alg)
            }
            ValidationError::InvalidTokenType {
                expected: _,
                got: _,
            } => "invalid_token_type".to_string(),
            ValidationError::JwksFetchError { url: _, error: _ } => "jwks_fetch_error".to_string(),
            ValidationError::InsufficientScopes { .. } => "insufficient_scopes".to_string(),
            ValidationError::TokenRevoked => "token_revoked".to_string(),
            ValidationError::StaleToken => "stale_token_version".to_string(),
            ValidationError::TokenStatusUnavailable => "token_status_unavailable".to_string(),
            ValidationError::InvalidTokenStatusClaims => "invalid_token_status_claims".to_string(),
            ValidationError::InvalidSecurityScheme { .. } => "invalid_security_scheme".to_string(),
        }
    }

    /// Log the error with structured logging
    fn log(&self) {
        match self {
            ValidationError::MissingToken => {
                debug!("JWT validation failed: missing token (no Authorization header or cookie)");
            }
            ValidationError::InvalidTokenFormat { error } => {
                warn!("JWT validation failed: invalid token format - {}", error);
            }
            ValidationError::MissingKeyId => {
                warn!("JWT validation failed: missing 'kid' (key ID) in token header");
            }
            ValidationError::MissingKey { kid } => {
                warn!(
                    "JWT validation failed: key not found for kid '{}' in JWKS",
                    kid
                );
            }
            ValidationError::InvalidSignature => {
                warn!("JWT validation failed: invalid signature");
            }
            ValidationError::ExpiredToken { exp, now } => {
                warn!(
                    "JWT validation failed: token expired (exp: {}, now: {})",
                    exp, now
                );
            }
            ValidationError::InvalidIssuer { expected, got } => {
                warn!(
                    "JWT validation failed: invalid issuer (expected: {:?}, got: {:?})",
                    expected, got
                );
            }
            ValidationError::InvalidAudience { expected, got } => {
                warn!(
                    "JWT validation failed: invalid audience (expected: {:?}, got: {:?})",
                    expected, got
                );
            }
            ValidationError::MissingRequiredClaim { claim } => {
                warn!("JWT validation failed: missing required claim '{}'", claim);
            }
            ValidationError::UnsupportedAlgorithm { alg } => {
                warn!("JWT validation failed: unsupported algorithm '{}'", alg);
            }
            ValidationError::InvalidTokenType { expected, got } => {
                warn!(
                    "JWT validation failed: invalid token type (expected: '{}', got: {:?})",
                    expected, got
                );
            }
            ValidationError::JwksFetchError { url, error } => {
                warn!(
                    "JWT validation failed: JWKS fetch error for {} - {}",
                    url, error
                );
            }
            ValidationError::InsufficientScopes { required, got } => {
                warn!(
                    "JWT validation failed: insufficient scopes (required: {:?}, got: {:?})",
                    required, got
                );
            }
            ValidationError::TokenRevoked => {
                warn!("JWT validation failed: token revoked");
            }
            ValidationError::StaleToken => {
                warn!("JWT validation failed: stale token version");
            }
            ValidationError::TokenStatusUnavailable => {
                warn!("JWT validation failed: token status dependency unavailable");
            }
            ValidationError::InvalidTokenStatusClaims => {
                warn!("JWT validation failed: required token-status claims invalid");
            }
            ValidationError::InvalidSecurityScheme { scheme } => {
                debug!(
                    "JWT validation failed: invalid security scheme '{}'",
                    scheme
                );
            }
        }
    }
}

/// Internal helper to validate a JWT token
///
/// Returns `bool` for backward compatibility, but uses structured error types
/// internally for better observability via logging.
/// Story 9.6: Structured JWT logging is called at all decision points.
pub(super) fn validate_token_impl(
    provider: &super::JwksBearerProvider,
    scheme: &SecurityScheme,
    scopes: &[String],
    req: &SecurityRequest,
) -> bool {
    match validate_token_internal(provider, scheme, scopes, req) {
        Ok(valid) => valid,
        Err(e) => {
            e.log();
            false
        }
    }
}

/// Extract and log structured JWT fields from claims.
///
/// Story 9.6: This function extracts all standard JWT fields from claims
/// and logs them via the provider's structured logger.
/// Returns None if claims extraction fails (malformed, no token, etc.).
fn extract_and_log_jwt_fields(
    provider: &super::JwksBearerProvider,
    token: &str,
    claims: &Value,
    decision_source: DecisionSource,
    result: &str,
    error_reason: Option<&str>,
) {
    let fields = JwtLogFields::from_claims(claims);
    let logger = &provider.structured_logger;

    match result {
        "allowed" => {
            logger.log_allowed(&fields, decision_source, None, None, token);
        }
        "denied" => {
            logger.log_denied(
                &fields,
                decision_source,
                None,
                None,
                error_reason.unwrap_or("unknown"),
                token,
            );
        }
        _ => {
            logger.log_failure(
                &fields,
                decision_source,
                None,
                None,
                error_reason.unwrap_or("unknown"),
                token,
            );
        }
    }
}

fn validate_dynamic_token_status(
    provider: &super::JwksBearerProvider,
    token: &str,
    claims: &Value,
) -> Result<(), ValidationError> {
    let (source, error, log_result) = match provider.check_token_status(claims) {
        JwtTokenStatus::Active => return Ok(()),
        JwtTokenStatus::Revoked => (
            DecisionSource::Denylist,
            ValidationError::TokenRevoked,
            "denied",
        ),
        JwtTokenStatus::Stale => (
            DecisionSource::VersionMismatch,
            ValidationError::StaleToken,
            "denied",
        ),
        JwtTokenStatus::Unavailable => (
            DecisionSource::TokenStatus,
            ValidationError::TokenStatusUnavailable,
            "failure",
        ),
        JwtTokenStatus::Invalid => (
            DecisionSource::TokenStatus,
            ValidationError::InvalidTokenStatusClaims,
            "failure",
        ),
    };
    let reason = error.error_reason();
    extract_and_log_jwt_fields(provider, token, claims, source, log_result, Some(&reason));
    Err(error)
}

/// Internal validation with structured error types
fn validate_token_internal(
    provider: &super::JwksBearerProvider,
    scheme: &SecurityScheme,
    scopes: &[String],
    req: &SecurityRequest,
) -> Result<bool, ValidationError> {
    // Check security scheme
    match scheme {
        SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
        SecurityScheme::Http { scheme, .. } => {
            return Err(ValidationError::InvalidSecurityScheme {
                scheme: scheme.clone(),
            });
        }
        _ => {
            return Err(ValidationError::InvalidSecurityScheme {
                scheme: format!("{:?}", scheme),
            });
        }
    }

    // Extract token
    let token = match provider.extract_token(req) {
        Some(t) => t,
        None => return Err(ValidationError::MissingToken),
    };

    // SECURITY: Parse header FIRST to get kid before cache lookup
    // This ensures cache key includes kid, so cache invalidates on key rotation
    let header = match jsonwebtoken::decode_header(token) {
        Ok(h) => h,
        Err(e) => {
            return Err(ValidationError::InvalidTokenFormat {
                error: format!("{:?}", e),
            });
        }
    };

    let kid = match header.kid {
        Some(k) => k,
        None => return Err(ValidationError::MissingKeyId),
    };

    // SECURITY: Enforce JWT typ claim (RFC 9068) - reject type confusion attacks
    // This check must occur AFTER header parsing but BEFORE any trust decision
    // Accepts only "at+jwt" for access tokens; rejects refresh tokens, API keys, ID tokens
    const EXPECTED_TYP: &str = "at+jwt";
    let header_typ = header.typ.as_deref().unwrap_or("");
    if header_typ != EXPECTED_TYP {
        return Err(ValidationError::InvalidTokenType {
            expected: EXPECTED_TYP.to_string(),
            got: if header_typ.is_empty() {
                None
            } else {
                Some(header_typ.to_string())
            },
        });
    }

    // SECURITY: The algorithm policy comes from trusted provider configuration, never from
    // the token alone. Consumers should configure the smallest issuer-specific allow-list.
    if !provider.algorithm_allowed(header.alg) {
        return Err(ValidationError::UnsupportedAlgorithm {
            alg: format!("{:?}", header.alg),
        });
    }

    // SECURITY: Include kid in cache key so cache invalidates on key rotation
    // Format: "token|kid" ensures different cache entries for same token with different keys
    let token_key: Arc<str> = Arc::from(format!("{}|{}", token, kid));

    // Check claims cache AFTER parsing header (we need kid for cache key)
    // SECURITY: On cache hit, verify key still exists in JWKS before using cached claims
    // This ensures tokens are invalidated when keys are rotated/revoked
    {
        // CRITICAL: Clone all needed values and release lock before calling get_key_for
        // get_key_for() can trigger HTTP requests (up to 400ms) via refresh_jwks_if_needed(),
        // which would block all other threads from accessing the claims cache
        let cached_data = {
            if let Ok(mut cache_guard) = provider.claims_cache.write() {
                if let Some((exp_timestamp_with_leeway, cached_claims, cached_kid)) =
                    cache_guard.get(&token_key)
                {
                    // Clone all values while holding the lock
                    Some((
                        *exp_timestamp_with_leeway,
                        cached_claims.clone(),
                        cached_kid.clone(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((exp_timestamp_clone, cached_claims_clone, cached_kid_clone)) = cached_data {
            // Lock is now released - safe to call get_key_for which may trigger HTTP requests
            // SECURITY: Verify the key still exists in JWKS (key rotation check)
            // If key was rotated, this will return None and we'll re-validate
            if provider.get_key_for(&cached_kid_clone).is_none() {
                // Key no longer exists (rotated/revoked), remove from cache
                debug!(
                    "JWT cache: key '{}' no longer in JWKS, invalidating cache entry",
                    cached_kid_clone
                );
                // Re-acquire lock to remove cache entry
                if let Ok(mut cache_guard) = provider.claims_cache.write() {
                    cache_guard.pop(&token_key);
                }
                // Fall through to full validation below
            } else {
                // Key still exists, check expiration
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                if now < exp_timestamp_clone {
                    // P2: Track cache hit
                    provider.cache_hits.fetch_add(1, Ordering::Relaxed);

                    validate_dynamic_token_status(provider, token, &cached_claims_clone)?;

                    // Story 9.6: Log cache-hit validation (decision_source = jwt_claims)
                    let token_scopes = cached_claims_clone
                        .get("scope")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let has_all_scopes = scopes
                        .iter()
                        .all(|s| token_scopes.split_whitespace().any(|ts| ts == s));

                    if has_all_scopes {
                        extract_and_log_jwt_fields(
                            provider,
                            token,
                            &cached_claims_clone,
                            DecisionSource::JwtClaims,
                            "allowed",
                            None,
                        );
                        return Ok(true);
                    } else {
                        let required: Vec<String> = scopes.to_vec();
                        let got: Vec<String> = token_scopes
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();
                        // Story 9.6: Log cache-hit denial (insufficient scopes)
                        extract_and_log_jwt_fields(
                            provider,
                            token,
                            &cached_claims_clone,
                            DecisionSource::JwtClaims,
                            "denied",
                            Some(&format!(
                                "insufficient_scopes: required={:?} got={:?}",
                                required, got
                            )),
                        );
                        return Err(ValidationError::InsufficientScopes { required, got });
                    }
                } else {
                    // Token expired, remove from cache
                    debug!("JWT cache: token expired, removing from cache");
                    // Re-acquire lock to remove expired entry
                    if let Ok(mut cache_guard) = provider.claims_cache.write() {
                        cache_guard.pop(&token_key);
                    }
                }
            }
        }
    }

    // Cache miss or key rotation detected - need to decode token
    // P2: Track cache miss
    provider.cache_misses.fetch_add(1, Ordering::Relaxed);

    // Calculate SystemTime only when needed (cache miss)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Get key for validation (will trigger JWKS refresh if needed)
    let key = match provider.get_key_for(&kid) {
        Some(k) => k,
        None => return Err(ValidationError::MissingKey { kid: kid.clone() }),
    };

    let selected_alg = header.alg;
    let mut validation = jsonwebtoken::Validation::new(selected_alg);
    validation.validate_exp = true;
    validation.set_required_spec_claims(&["exp"]);
    validation.leeway = provider.leeway_secs;
    if let Some(ref iss) = provider.iss {
        validation.set_issuer(&[iss]);
    }
    if let Some(ref aud) = provider.aud {
        validation.set_audience(&[aud]);
    }
    let data: Result<jsonwebtoken::TokenData<Value>, jsonwebtoken::errors::Error> =
        jsonwebtoken::decode(token, &key, &validation);
    let claims = match data {
        Ok(d) => d.claims,
        Err(e) => {
            // Map jsonwebtoken errors to our structured error types
            let error_result = match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    // Try to extract exp from the error if available
                    let exp = now; // Default to now if we can't extract
                    ValidationError::ExpiredToken { exp, now }
                }
                jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                    ValidationError::InvalidSignature
                }
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                    ValidationError::InvalidIssuer {
                        expected: provider.iss.clone(),
                        got: None, // jsonwebtoken doesn't provide the actual value
                    }
                }
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    ValidationError::InvalidAudience {
                        expected: provider.aud.clone(),
                        got: None, // jsonwebtoken doesn't provide the actual value
                    }
                }
                jsonwebtoken::errors::ErrorKind::MissingRequiredClaim(claim) => {
                    ValidationError::MissingRequiredClaim {
                        claim: claim.clone(),
                    }
                }
                _ => ValidationError::InvalidTokenFormat {
                    error: format!("{:?}", e),
                },
            };

            // Story 9.6: Log validation failure with error details
            // We cannot extract claims from the failed token, so we log minimal info
            debug!(
                "JWT validation failed: {:?} (token not cached, will not log structured fields)",
                error_result
            );

            return Err(error_result);
        }
    };

    validate_dynamic_token_status(provider, token, &claims)?;

    // P0: Store decoded claims in cache with leeway applied to expiration
    // Extract exp claim to determine cache validity
    if let Some(exp_value) = claims.get("exp") {
        if let Some(exp_timestamp) = exp_value.as_i64() {
            // P0: Store expiration WITH leeway to match validation logic
            let exp_timestamp_with_leeway = exp_timestamp + provider.leeway_secs as i64;

            // Only cache if token hasn't expired (with leeway)
            // SECURITY: Store kid with cached claims so we can verify key existence on cache hits
            if now < exp_timestamp_with_leeway {
                if let Ok(mut cache_guard) = provider.claims_cache.write() {
                    // P0: Use Arc<str> key (already created above with kid included)
                    // P1: Write lock for cache insert (eviction may occur)
                    // P2: Track evictions correctly - LruCache::put() returns Some(old_value) when
                    //     updating an existing key, NOT when evicting. To detect evictions, we must
                    //     check if the key doesn't exist AND the cache is at capacity before inserting.
                    let key_exists = cache_guard.peek(&token_key).is_some();
                    let cache_at_capacity = cache_guard.len() >= cache_guard.cap().get();
                    let will_evict = !key_exists && cache_at_capacity;

                    // Insert/update the cache entry
                    cache_guard.put(token_key, (exp_timestamp_with_leeway, claims.clone(), kid));

                    // Track eviction only if we inserted a new key when cache was at capacity
                    if will_evict {
                        provider.cache_evictions.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    // scope check
    let token_scopes = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
    let has_all_scopes = scopes
        .iter()
        .all(|s| token_scopes.split_whitespace().any(|ts| ts == s));

    if has_all_scopes {
        // Story 9.6: Log successful cache-miss validation
        extract_and_log_jwt_fields(
            provider,
            token,
            &claims,
            DecisionSource::JwtClaims,
            "allowed",
            None,
        );
        debug!("JWT validation succeeded: token valid, scopes present");
        Ok(true)
    } else {
        let required: Vec<String> = scopes.to_vec();
        let got: Vec<String> = token_scopes
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        // Story 9.6: Log cache-miss denial (insufficient scopes)
        extract_and_log_jwt_fields(
            provider,
            token,
            &claims,
            DecisionSource::JwtClaims,
            "denied",
            Some(&format!(
                "insufficient_scopes: required={:?} got={:?}",
                required, got
            )),
        );
        Err(ValidationError::InsufficientScopes { required, got })
    }
}

/// Internal helper to extract JWT claims
pub(super) fn extract_claims_impl(
    provider: &super::JwksBearerProvider,
    scheme: &SecurityScheme,
    req: &SecurityRequest,
) -> Option<Value> {
    // SecurityProvider::extract_claims is called only after validate() succeeds. Dynamic status
    // is therefore deliberately checked by validate_token_internal, once per authorization
    // attempt. Rechecking here would either double the authoritative lookup on every request or
    // require consumers to negative-cache Active and create a revocation window.
    match scheme {
        SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
        _ => return None,
    }

    let token = provider.extract_token(req)?;

    // Parse header to get kid for cache key
    let header = match jsonwebtoken::decode_header(token) {
        Ok(h) => h,
        Err(_) => return None,
    };

    // SECURITY: Enforce JWT typ claim (RFC 9068) - reject type confusion attacks
    // Same check as validate_token_internal for consistency
    const EXPECTED_TYP: &str = "at+jwt";
    if header.typ.as_deref() != Some(EXPECTED_TYP) {
        return None;
    }

    if !provider.algorithm_allowed(header.alg) {
        return None;
    }

    let kid = header.kid?;

    // Check cache first
    // SECURITY: On cache hit, verify key still exists in JWKS before using cached claims
    // This ensures tokens are invalidated when keys are rotated/revoked
    let token_key: Arc<str> = Arc::from(format!("{}|{}", token, kid));
    {
        // CRITICAL: Clone all needed values and release lock before calling get_key_for
        // get_key_for() can trigger HTTP requests (up to 400ms) via refresh_jwks_if_needed(),
        // which would block all other threads from accessing the claims cache
        let cached_data = {
            if let Ok(mut cache_guard) = provider.claims_cache.write() {
                if let Some((exp_timestamp_with_leeway, cached_claims, cached_kid)) =
                    cache_guard.get(&token_key)
                {
                    // Clone all values while holding the lock
                    Some((
                        *exp_timestamp_with_leeway,
                        cached_claims.clone(),
                        cached_kid.clone(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((exp_timestamp_clone, cached_claims_clone, cached_kid_clone)) = cached_data {
            // Lock is now released - safe to call get_key_for which may trigger HTTP requests
            // SECURITY: Verify the key still exists in JWKS (key rotation check)
            // If key was rotated, this will return None and we'll re-validate
            if provider.get_key_for(&cached_kid_clone).is_none() {
                // Key no longer exists (rotated/revoked), remove from cache
                debug!(
                    "JWT cache: key '{}' no longer in JWKS, invalidating cache entry for claims extraction",
                    cached_kid_clone
                );
                // Re-acquire lock to remove cache entry
                if let Ok(mut cache_guard) = provider.claims_cache.write() {
                    cache_guard.pop(&token_key);
                }
                // Fall through to full validation below
            } else {
                // Key still exists, check expiration
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                if now < exp_timestamp_clone {
                    // P2: Track cache hit
                    provider.cache_hits.fetch_add(1, Ordering::Relaxed);
                    return Some(cached_claims_clone);
                } else {
                    // Token expired, remove from cache
                    debug!("JWT cache: token expired for claims extraction, removing from cache");
                    // Re-acquire lock to remove expired entry
                    if let Ok(mut cache_guard) = provider.claims_cache.write() {
                        cache_guard.pop(&token_key);
                    }
                }
            }
        }
    }

    // Cache miss - validate and decode token
    provider.cache_misses.fetch_add(1, Ordering::Relaxed);

    // Calculate SystemTime only when needed (cache miss)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Get key for validation
    let key = match provider.get_key_for(&kid) {
        Some(k) => k,
        None => return None,
    };

    let selected_alg = header.alg;
    let mut validation = jsonwebtoken::Validation::new(selected_alg);
    validation.validate_exp = true;
    validation.set_required_spec_claims(&["exp"]);
    validation.leeway = provider.leeway_secs;
    if let Some(ref iss) = provider.iss {
        validation.set_issuer(&[iss]);
    }
    if let Some(ref aud) = provider.aud {
        validation.set_audience(&[aud]);
    }
    let data: Result<jsonwebtoken::TokenData<Value>, jsonwebtoken::errors::Error> =
        jsonwebtoken::decode(token, &key, &validation);
    let claims = match data {
        Ok(d) => d.claims,
        Err(e) => {
            debug!("JWT claims extraction failed: {:?}", e);
            return None;
        }
    };

    // Store in cache
    if let Some(exp_value) = claims.get("exp") {
        if let Some(exp_timestamp) = exp_value.as_i64() {
            let exp_timestamp_with_leeway = exp_timestamp + provider.leeway_secs as i64;
            if now < exp_timestamp_with_leeway {
                if let Ok(mut cache_guard) = provider.claims_cache.write() {
                    cache_guard.put(token_key, (exp_timestamp_with_leeway, claims.clone(), kid));
                }
            }
        }
    }

    Some(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::jwks_bearer::JwtLogFields;

    /// Unit: JwtLogFields extracts all fields from claims.
    #[test]
    fn test_extract_jwt_log_fields() {
        let claims = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123",
            "aud": "web-portal",
            "sid": "ses_01JV8W",
            "jti": "tok_abc123",
            "ver": 42,
            "act": {
                "sub": "support_agent_456"
            }
        });

        let fields = JwtLogFields::from_claims(&claims);
        assert_eq!(fields.issuer, Some("https://idam.example.com".to_string()));
        assert_eq!(fields.subject, Some("user_123".to_string()));
        assert_eq!(fields.client_id, Some("web-portal".to_string()));
        assert_eq!(fields.session_id, Some("ses_01JV8W".to_string()));
        assert_eq!(fields.token_id, Some("tok_abc123".to_string()));
        assert_eq!(fields.token_version, Some(42));
        assert_eq!(fields.actor_subject, Some("support_agent_456".to_string()));
    }

    /// Unit: JwtLogFields handles missing claims.
    #[test]
    fn test_extract_jwt_log_fields_minimal() {
        let claims = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123"
        });

        let fields = JwtLogFields::from_claims(&claims);
        assert_eq!(fields.issuer, Some("https://idam.example.com".to_string()));
        assert_eq!(fields.subject, Some("user_123".to_string()));
        assert_eq!(fields.client_id, None);
        assert_eq!(fields.session_id, None);
        assert_eq!(fields.token_id, None);
        assert_eq!(fields.token_version, None);
        assert_eq!(fields.actor_subject, None);
    }

    /// Security: No raw token in fields.
    #[test]
    fn test_no_raw_token_in_fields() {
        let claims = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123"
        });
        let fields = JwtLogFields::from_claims(&claims);
        let raw_token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert!(fields.validate_no_raw_token(raw_token));
    }

    /// Security: No PII in fields.
    #[test]
    fn test_no_pii_in_fields() {
        let claims = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123"
        });
        let fields = JwtLogFields::from_claims(&claims);
        assert!(fields.validate_no_pii());
    }

    /// Unit: DecisionSource has correct string representations.
    #[test]
    fn test_decision_source_strings() {
        assert_eq!(DecisionSource::JwtClaims.as_str(), "jwt_claims");
        assert_eq!(DecisionSource::FallbackCached.as_str(), "fallback_cached");
        assert_eq!(DecisionSource::FallbackOnline.as_str(), "fallback_online");
        assert_eq!(DecisionSource::Denylist.as_str(), "denylist");
        assert_eq!(DecisionSource::VersionMismatch.as_str(), "version_mismatch");
        assert_eq!(DecisionSource::OnlineOnly.as_str(), "online_only");
    }

    /// Unit: ValidationError has error_reason method.
    #[test]
    fn test_validation_error_reason() {
        let e = ValidationError::MissingToken;
        assert_eq!(e.error_reason(), "missing_token");

        let e = ValidationError::InvalidSignature;
        assert_eq!(e.error_reason(), "invalid_signature");

        let e = ValidationError::ExpiredToken { exp: 100, now: 200 };
        assert_eq!(e.error_reason(), "token_expired");
    }

    /// Unit: JwtLogFields from empty claims returns all None.
    #[test]
    fn test_extract_fields_empty() {
        let fields = JwtLogFields::from_claims(&serde_json::Value::Null);
        assert_eq!(fields.issuer, None);
        assert_eq!(fields.subject, None);
        assert_eq!(fields.client_id, None);
        assert_eq!(fields.session_id, None);
        assert_eq!(fields.token_id, None);
        assert_eq!(fields.token_version, None);
        assert_eq!(fields.actor_subject, None);
    }

    /// Unit: actor_subject extracted from act.claim properly.
    #[test]
    fn test_actor_subject_from_act() {
        let claims = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123",
            "act": {
                "sub": "support_agent_456",
                "roles": ["admin"]
            }
        });
        let fields = JwtLogFields::from_claims(&claims);
        assert_eq!(fields.actor_subject, Some("support_agent_456".to_string()));
    }

    /// Unit: client_id uses aud first, falls back to client_id.
    #[test]
    fn test_client_id_priority() {
        let claims_with_aud = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123",
            "aud": "web-portal",
            "client_id": "mobile-app"
        });
        let fields = JwtLogFields::from_claims(&claims_with_aud);
        assert_eq!(fields.client_id, Some("web-portal".to_string()));

        let claims_with_client_id = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123",
            "client_id": "mobile-app"
        });
        let fields = JwtLogFields::from_claims(&claims_with_client_id);
        assert_eq!(fields.client_id, Some("mobile-app".to_string()));
    }

    /// Unit: token_version as number vs string.
    #[test]
    fn test_token_version_types() {
        let claims_number = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123",
            "ver": 42
        });
        let fields = JwtLogFields::from_claims(&claims_number);
        assert_eq!(fields.token_version, Some(42));

        let claims_string = serde_json::json!({
            "iss": "https://idam.example.com",
            "sub": "user_123",
            "ver": "42"
        });
        let fields = JwtLogFields::from_claims(&claims_string);
        assert_eq!(fields.token_version, None); // strings are not parsed as u64
    }
}
