//! Token revocation checking for SPIFFE SVIDs
//!
//! This module provides interfaces and implementations for checking if a JWT ID (jti)
//! has been revoked. Supports multiple backends: in-memory, Redis, database, and external services.

use std::sync::Arc;

/// Trait for checking if a token has been revoked
///
/// Implementations can use various backends:
/// - In-memory cache (for testing or small deployments)
/// - Redis (for distributed revocation lists)
/// - Database (for persistent revocation)
/// - External service (for centralized revocation management)
pub trait RevocationChecker: Send + Sync {
    /// Check if a JWT ID (jti) has been revoked
    ///
    /// # Arguments
    ///
    /// * `jti` - The JWT ID to check
    ///
    /// # Returns
    ///
    /// * `true` - Token is revoked (should be rejected)
    /// * `false` - Token is not revoked (or revocation check unavailable)
    fn is_revoked(&self, jti: &str) -> bool;
}

/// In-memory revocation checker (for testing or small deployments)
///
/// Stores revoked JTI values in a HashSet. Not suitable for production
/// multi-instance deployments as revocation state is not shared.
#[derive(Clone)]
pub struct InMemoryRevocationChecker {
    revoked: Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
}

impl InMemoryRevocationChecker {
    /// Create a new in-memory revocation checker
    pub fn new() -> Self {
        Self {
            revoked: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// Revoke a JWT ID
    pub fn revoke(&self, jti: &str) {
        if let Ok(mut guard) = self.revoked.write() {
            guard.insert(jti.to_string());
        }
    }

    /// Unrevoke a JWT ID (for testing)
    pub fn unrevoke(&self, jti: &str) {
        if let Ok(mut guard) = self.revoked.write() {
            guard.remove(jti);
        }
    }

    /// Clear all revocations (for testing)
    pub fn clear(&self) {
        if let Ok(mut guard) = self.revoked.write() {
            guard.clear();
        }
    }
}

impl Default for InMemoryRevocationChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RevocationChecker for InMemoryRevocationChecker {
    fn is_revoked(&self, jti: &str) -> bool {
        self.revoked
            .read()
            .map(|guard| guard.contains(jti))
            .unwrap_or(false)
    }
}

/// No-op revocation checker (always returns false)
///
/// Use this when revocation checking is disabled or not yet implemented.
#[derive(Clone, Default)]
pub struct NoOpRevocationChecker;

impl RevocationChecker for NoOpRevocationChecker {
    fn is_revoked(&self, _jti: &str) -> bool {
        false
    }
}
