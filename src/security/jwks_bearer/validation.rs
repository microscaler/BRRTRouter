//! JWT validation and claims extraction logic
//!
//! This module contains the core validation logic for JWKS-based JWT validation,
//! including token validation, claims extraction, and cache management.

use crate::security::SecurityRequest;
use crate::spec::SecurityScheme;
use jsonwebtoken;
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{debug, warn};

// Re-export constants from parent module
use super::SUPPORTED_ALGORITHMS;

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
    InvalidIssuer { expected: Option<String>, got: Option<String> },
    /// Token audience doesn't match expected value
    InvalidAudience { expected: Option<String>, got: Option<String> },
    /// Token is missing a required claim
    MissingRequiredClaim { claim: String },
    /// Token uses an unsupported algorithm
    UnsupportedAlgorithm { alg: String },
    /// JWKS fetch failed
    #[allow(dead_code)] // Reserved for future use when JWKS fetch errors are tracked
    JwksFetchError { url: String, error: String },
    /// Token is missing required scopes
    InsufficientScopes { required: Vec<String>, got: Vec<String> },
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
            ValidationError::JwksFetchError { .. } => "JWKS fetch failed",
            ValidationError::InsufficientScopes { .. } => "insufficient scopes",
            ValidationError::InvalidSecurityScheme { .. } => "invalid security scheme",
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
                warn!("JWT validation failed: key not found for kid '{}' in JWKS", kid);
            }
            ValidationError::InvalidSignature => {
                warn!("JWT validation failed: invalid signature");
            }
            ValidationError::ExpiredToken { exp, now } => {
                warn!("JWT validation failed: token expired (exp: {}, now: {})", exp, now);
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
            ValidationError::JwksFetchError { url, error } => {
                warn!("JWT validation failed: JWKS fetch error for {} - {}", url, error);
            }
            ValidationError::InsufficientScopes { required, got } => {
                warn!(
                    "JWT validation failed: insufficient scopes (required: {:?}, got: {:?})",
                    required, got
                );
            }
            ValidationError::InvalidSecurityScheme { scheme } => {
                debug!("JWT validation failed: invalid security scheme '{}'", scheme);
            }
        }
    }
}

/// Internal helper to validate a JWT token
///
/// Returns `bool` for backward compatibility, but uses structured error types
/// internally for better observability via logging.
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

                    // SECURITY: Key verified, expiration checked, use cached claims
                    // Note: We skip signature/issuer/audience re-validation here for performance,
                    // but the key existence check ensures rotation is detected
                    let token_scopes = cached_claims_clone
                        .get("scope")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let has_all_scopes = scopes
                        .iter()
                        .all(|s| token_scopes.split_whitespace().any(|ts| ts == s));

                    if has_all_scopes {
                        debug!("JWT validation succeeded: cache hit, key verified, scopes valid");
                        return Ok(true);
                    } else {
                        let required: Vec<String> = scopes.iter().cloned().collect();
                        let got: Vec<String> = token_scopes
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();
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

    // P4 Security: Only allow supported algorithms (whitelist approach for security)
    // P3: Simplified algorithm selection using whitelist instead of verbose match
    if !SUPPORTED_ALGORITHMS.contains(&header.alg) {
        return Err(ValidationError::UnsupportedAlgorithm {
            alg: format!("{:?}", header.alg),
        });
    }
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
            return Err(match e.kind() {
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
            });
        }
    };

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
                    cache_guard.put(
                        token_key.clone(),
                        (exp_timestamp_with_leeway, claims.clone(), kid.clone()),
                    );

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
        debug!("JWT validation succeeded: token valid, scopes present");
        Ok(true)
    } else {
        let required: Vec<String> = scopes.iter().cloned().collect();
        let got: Vec<String> = token_scopes
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        Err(ValidationError::InsufficientScopes { required, got })
    }
}

/// Internal helper to extract JWT claims
pub(super) fn extract_claims_impl(
    provider: &super::JwksBearerProvider,
    scheme: &SecurityScheme,
    req: &SecurityRequest,
) -> Option<Value> {
    match scheme {
        SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
        _ => return None,
    }

    let token = match provider.extract_token(req) {
        Some(t) => t,
        None => return None,
    };

    // Parse header to get kid for cache key
    let header = match jsonwebtoken::decode_header(token) {
        Ok(h) => h,
        Err(_) => return None,
    };

    let kid = match header.kid {
        Some(k) => k,
        None => return None,
    };

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
                    "JWT cache: key '{}' no longer in JWKS, invalidating cache entry",
                    cached_kid_clone
                );
                // Re-acquire lock to remove cache entry
                if let Ok(mut cache_guard) = provider.claims_cache.write() {
                    cache_guard.pop(&token_key);
                }
                // Fall through to full decode below
            } else {
                // Key still exists, check expiration
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                if now < exp_timestamp_clone {
                    // SECURITY: Key verified, expiration checked, use cached claims
                    // Note: We skip signature/issuer/audience re-validation here for performance,
                    // but the key existence check ensures rotation is detected
                    return Some(cached_claims_clone);
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

    // Cache miss - decode token (same logic as validate, but we return claims)
    let key = match provider.get_key_for(&kid) {
        Some(k) => k,
        None => return None,
    };

    // P3: Simplified algorithm selection using whitelist
    if !SUPPORTED_ALGORITHMS.contains(&header.alg) {
        return None;
    }
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

    match jsonwebtoken::decode(token, &key, &validation) {
        Ok(data) => Some(data.claims),
        Err(_) => None,
    }
}
