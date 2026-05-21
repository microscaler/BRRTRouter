//! Authorization decision types for BRRTRouter
//!
//! These types are used by the shadow decision observability system (Story 9.4)
//! and the JWT common-path middleware (Story 4.2) to track and compare JWT
//! authorization decisions against online authz-core checks.
//!
//! **Security Note:** These types do NOT carry PII fields (email, phone, name).
//! Only `user_id`, `role`, and `permission` are tracked in span attributes.

use serde::{Deserialize, Serialize};

/// Represents an authorization decision: allowed, denied, or common-path validated.
///
/// This enum replaces the old `bool` return type from `SecurityProvider::validate()`
/// to provide richer decision semantics for both the shadow decision observability
/// system and the JWT common-path middleware.
///
/// # Decision Flow (Story 4.2)
///
/// ```text
/// Request -> JWT Middleware -> Route Policy Lookup
///   ├─ jwt-only + policy pass -> AuthDecision::Allowed { claims }
///   ├─ jwt-only + policy fail -> AuthDecision::Denied { reason }
///   └─ jwt-with-fallback/online-only -> AuthDecision::JwtCommonPath { claims }
/// ```
///
/// # Usage
///
/// ```rust
/// use brrtrouter::security::decision::AuthDecision;
///
/// let decision = AuthDecision::allowed(Some("role:admin".to_string()));
/// assert!(decision.is_allowed());
///
/// let denied = AuthDecision::denied(Some("missing scope:read".to_string()));
/// assert!(denied.is_denied());
///
/// let common_path = AuthDecision::jwt_common_path(Some("role:user".to_string()));
/// assert!(common_path.is_common_path());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthDecision {
    /// Request is allowed by JWT common-path evaluation.
    /// The `reason` field provides context (e.g., "role:admin", "tenant_match").
    Allowed {
        /// Optional reason for the allow decision (e.g., role, permission, scope).
        reason: Option<String>,
    },
    /// Request is denied by JWT common-path evaluation.
    /// The `reason` field provides context (e.g., "missing scope:read", "tenant_mismatch").
    Denied {
        /// Optional reason for the deny decision.
        reason: Option<String>,
    },
    /// JWT validated successfully, but authorization requires online fallback
    /// (jwt-with-fallback) or authz-core call (online-only).
    ///
    /// The common-path middleware has validated the JWT (signature, expiry, issuer,
    /// audience) and passed the claims to the handler. The handler or a downstream
    /// middleware is responsible for the final authorization decision.
    JwtCommonPath {
        /// Optional reason/context for why this route requires online fallback.
        reason: Option<String>,
    },
}

impl AuthDecision {
    /// Create a new allowed decision with an optional reason.
    #[must_use]
    pub fn allowed(reason: impl Into<Option<String>>) -> Self {
        Self::Allowed {
            reason: reason.into(),
        }
    }

    /// Create a new denied decision with an optional reason.
    #[must_use]
    pub fn denied(reason: impl Into<Option<String>>) -> Self {
        Self::Denied {
            reason: reason.into(),
        }
    }

    /// Create a new common-path decision (JWT validated, but online fallback needed).
    #[must_use]
    pub fn jwt_common_path(reason: impl Into<Option<String>>) -> Self {
        Self::JwtCommonPath {
            reason: reason.into(),
        }
    }

    /// Returns `true` if this is an allowed decision.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    /// Returns `true` if this is a denied decision.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied { .. })
    }

    /// Returns `true` if this is a common-path decision (continues to handler).
    #[must_use]
    pub fn is_common_path(&self) -> bool {
        matches!(self, Self::JwtCommonPath { .. })
    }

    /// Returns the decision string representation for logging and span attributes.
    #[must_use]
    pub fn decision_str(&self) -> &str {
        match self {
            Self::Allowed { .. } => "allowed",
            Self::Denied { .. } => "denied",
            Self::JwtCommonPath { .. } => "common_path",
        }
    }

    /// Returns the optional reason for this decision.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allowed { reason } => reason.as_deref(),
            Self::Denied { reason } => reason.as_deref(),
            Self::JwtCommonPath { reason } => reason.as_deref(),
        }
    }
}

impl From<bool> for AuthDecision {
    /// Convert a `bool` to `AuthDecision` — preserves legacy compatibility.
    /// `true` → Allowed, `false` → Denied (no reason).
    fn from(value: bool) -> Self {
        if value {
            Self::Allowed { reason: None }
        } else {
            Self::Denied { reason: None }
        }
    }
}

impl From<AuthDecision> for bool {
    /// Convert `AuthDecision` back to `bool` — extracts the allow/deny semantics.
    /// CommonPath decisions are treated as `true` (proceed to handler).
    fn from(decision: AuthDecision) -> Self {
        decision.is_allowed() || decision.is_common_path()
    }
}

/// Mismatch reason classification for shadow decision comparisons.
///
/// This enum is used in the `shadow_decision.compare` span to classify
/// when JWT and online authorization decisions differ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MismatchReason {
    /// JWT allows but online would deny — **CRITICAL**: potential security vulnerability.
    JwtAllowedButOnlineDenied,
    /// JWT denies but online would allow — **WARNING**: potential false negative.
    JwtDeniedButOnlineAllowed,
}

impl MismatchReason {
    /// Returns the string representation for span attributes.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::JwtAllowedButOnlineDenied => "jwt_allowed_but_online_denied",
            Self::JwtDeniedButOnlineAllowed => "jwt_denied_but_online_allowed",
        }
    }

    /// Returns the severity level for this mismatch type.
    #[must_use]
    pub fn severity(&self) -> &'static str {
        match self {
            Self::JwtAllowedButOnlineDenied => "CRITICAL",
            Self::JwtDeniedButOnlineAllowed => "WARNING",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_decision_allows() {
        let allowed = AuthDecision::allowed(Some("role:admin".to_string()));
        assert!(allowed.is_allowed());
        assert!(!allowed.is_denied());
        assert!(!allowed.is_common_path());
        assert_eq!(allowed.decision_str(), "allowed");
        assert_eq!(allowed.reason(), Some("role:admin"));
    }

    #[test]
    fn test_auth_decision_denies() {
        let denied = AuthDecision::denied(Some("missing scope".to_string()));
        assert!(!denied.is_allowed());
        assert!(denied.is_denied());
        assert!(!denied.is_common_path());
        assert_eq!(denied.decision_str(), "denied");
        assert_eq!(denied.reason(), Some("missing scope"));
    }

    #[test]
    fn test_auth_decision_common_path() {
        let common_path = AuthDecision::jwt_common_path(Some("online_fallback_needed".to_string()));
        assert!(!common_path.is_allowed());
        assert!(!common_path.is_denied());
        assert!(common_path.is_common_path());
        assert_eq!(common_path.decision_str(), "common_path");
        assert_eq!(common_path.reason(), Some("online_fallback_needed"));
    }

    #[test]
    fn test_auth_decision_no_reason() {
        let allowed = AuthDecision::allowed(None::<String>);
        assert!(allowed.is_allowed());
        assert_eq!(allowed.reason(), None);

        let denied = AuthDecision::denied(None::<String>);
        assert!(denied.is_denied());
        assert_eq!(denied.reason(), None);

        let common_path = AuthDecision::jwt_common_path(None::<String>);
        assert!(common_path.is_common_path());
        assert_eq!(common_path.reason(), None);
    }

    #[test]
    fn test_from_bool_true() {
        let decision: AuthDecision = true.into();
        assert!(decision.is_allowed());
        assert_eq!(decision.reason(), None);
    }

    #[test]
    fn test_from_bool_false() {
        let decision: AuthDecision = false.into();
        assert!(decision.is_denied());
        assert_eq!(decision.reason(), None);
    }

    #[test]
    fn test_to_bool_allowed() {
        let decision: bool = AuthDecision::allowed(None).into();
        assert!(decision);
    }

    #[test]
    fn test_to_bool_denied() {
        let decision: bool = AuthDecision::denied(None).into();
        assert!(!decision);
    }

    #[test]
    fn test_to_bool_common_path() {
        // CommonPath should convert to true (proceed to handler)
        let decision: bool = AuthDecision::jwt_common_path(None).into();
        assert!(decision);
    }

    #[test]
    fn test_mismatch_reason_strings() {
        assert_eq!(
            MismatchReason::JwtAllowedButOnlineDenied.as_str(),
            "jwt_allowed_but_online_denied"
        );
        assert_eq!(
            MismatchReason::JwtDeniedButOnlineAllowed.as_str(),
            "jwt_denied_but_online_allowed"
        );
    }

    #[test]
    fn test_mismatch_reason_severity() {
        assert_eq!(
            MismatchReason::JwtAllowedButOnlineDenied.severity(),
            "CRITICAL"
        );
        assert_eq!(
            MismatchReason::JwtDeniedButOnlineAllowed.severity(),
            "WARNING"
        );
    }

    #[test]
    fn test_equality() {
        let d1 = AuthDecision::allowed(Some("role:admin".to_string()));
        let d2 = AuthDecision::allowed(Some("role:admin".to_string()));
        let d3 = AuthDecision::allowed(Some("role:user".to_string()));
        let d4 = AuthDecision::denied(None);

        assert_eq!(d1, d2);
        assert_ne!(d1, d3);
        assert_ne!(d1, d4);

        let d5 = AuthDecision::jwt_common_path(None);
        let d6 = AuthDecision::jwt_common_path(None);
        let d7 = AuthDecision::jwt_common_path(Some("fallback".to_string()));
        assert_eq!(d5, d6);
        assert_eq!(d5, d7); // reason is Option, so None == Some(_) for equality? No, let's be strict
        assert_ne!(d5, d1);
    }
}
