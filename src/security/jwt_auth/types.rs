//! JWT common-path authorization middleware types for Story 4.2
//!
//! This module defines the core types needed for JWT common-path authorization:
//! - `JwtAuthMiddleware` - the middleware that validates JWTs and evaluates local policy
//! - `AuthError` - error types for JWT validation failures
//! - `AccessClaims` - the decoded JWT claims structure
//! - `RoutePolicyStore`, `RoutePolicy`, `RouteAuthCategory` - route classification
//! - `JwksClient` - JWKS client interface for JWT validation
//!
//! **Design Context (Story 4.2):**
//!
//! The JWT document's core thesis: JWT claims handle the common path, with online
//! fallback for high-risk decisions. This middleware implements the JWT common-path
//! evaluation that replaces the current per-request authz-core call for `jwt-only` routes.
//!
//! **Middleware Placement:**
//!
//! ```text
//! Client Request
//!   -> BRRTRouter Router (path matching)
//!     -> JWT Common-Path Middleware  <-- NEW
//!       -> If jwt-only: evaluate claims, return allow/deny
//!       -> If jwt-with-fallback or online-only: continue to handler
//!     -> Handler (business logic)
//! ```
//!
//! **Security Considerations:**
//!
//! - Tenant validation is the most critical check (HACK-401, HACK-403)
//! - JWT signature validation is the ONLY defense (HACK-402)
//! - JWKS cache poisoning must be prevented (HACK-404)
//! - NEVER fail open on middleware errors (HACK-405)

use serde::{Deserialize, Serialize};

/// JWT claims extracted from a validated Access Token (at+jwt).
///
/// This struct represents the decoded claims from a JWT token that has been
/// validated by the JWKS client (signature, expiry, issuer, audience checks).
///
/// The claims structure follows the Sesame-IDAM token format where the `sx`
/// field contains authorization-relevant data (roles, permissions, risk level).
///
/// # Security Note
///
/// These claims are only populated AFTER successful JWT validation. The JWT
/// signature is the foundational security check — without a valid signature,
/// no claims are trusted.
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::jwt_auth::AccessClaims;
///
/// let claims = AccessClaims {
///     sub: "user-123".to_string(),
///     tenant_id: "tenant-abc".to_string(),
///     user_type: "customer".to_string(),
///     sx: SxClaims {
///         roles: vec!["admin".to_string()],
///         permissions: vec!["users:read".to_string()],
///         risk: Some("normal".to_string()),
///     },
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Subject identifier (unique user/entity ID)
    pub sub: String,
    /// Tenant identifier — must match X-Tenant-ID header (HACK-401)
    pub tenant_id: String,
    /// User type: "customer" or "platform"
    pub user_type: String,
    /// Authorization claims (roles, permissions, risk level)
    pub sx: SxClaims,
}

impl AccessClaims {
    /// Create a new AccessClaims from a validated JWT payload.
    ///
    /// This should only be called after successful JWT validation by the
    /// JWKS client. Claims from unvalidated tokens are NOT trusted.
    pub fn from_payload(payload: serde_json::Value) -> Option<Self> {
        serde_json::from_value(payload).ok()
    }
}

/// Authorization claims nested within the JWT claims structure.
///
/// The `sx` field contains all authorization-relevant data that the
/// JWT common-path middleware evaluates for jwt-only routes.
///
/// # Fields
///
/// - `roles`: User roles (e.g., "admin", "org_admin", "customer")
/// - `permissions`: Fine-grained permissions (e.g., "users:read", "prefs:write")
/// - `risk`: Risk level of the token ("normal", "elevated", "critical")
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SxClaims {
    /// User roles (empty array is valid — no roles granted)
    pub roles: Vec<String>,
    /// User permissions (empty array is valid — no permissions granted)
    pub permissions: Vec<String>,
    /// Optional risk level (absence of risk claim does not cause denial)
    pub risk: Option<String>,
}

impl SxClaims {
    /// Check if the user has a specific role.
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Check if the user has a specific permission.
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// Check if the risk level is "normal" or None (safe).
    #[must_use]
    pub fn is_normal_risk(&self) -> bool {
        self.risk.as_deref() != Some("elevated") && self.risk.as_deref() != Some("critical")
    }

    /// Check if any roles or permissions are present.
    #[must_use]
    pub fn has_any_authorization(&self) -> bool {
        !self.roles.is_empty() || !self.permissions.is_empty()
    }
}

/// Authorization errors that can occur during JWT common-path middleware processing.
///
/// These errors are returned when JWT validation or policy evaluation fails.
/// Each error type maps to a specific HTTP response code.
///
/// # Error Response Codes
///
/// - `MissingAuthorization` → 401 Unauthorized
/// - `InvalidBearerScheme` → 401 Unauthorized
/// - `TokenExpired` → 401 Unauthorized
/// - `TokenInvalid` → 401 Unauthorized
/// - `MissingTenantId` → 401 Unauthorized
/// - `TenantMismatch` → 401 Unauthorized
/// - `PolicyNotFound` → 503 Service Unavailable (fail closed)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthError {
    /// Missing Authorization header
    MissingAuthorization,
    /// Authorization header is present but not a Bearer token
    InvalidBearerScheme {
        /// The actual scheme used (e.g., "Basic")
        scheme: String,
    },
    /// JWT token has expired (exp claim in the past)
    TokenExpired {
        /// The expiry timestamp that was exceeded
        expired_at: i64,
    },
    /// JWT token is invalid (signature mismatch, malformed, etc.)
    TokenInvalid,
    /// Missing X-Tenant-ID header (critical for tenant isolation)
    MissingTenantId,
    /// JWT claims tenant_id does not match request X-Tenant-ID header
    ///
    /// This is the most critical security check — if it fails, the request
    /// must be rejected immediately to prevent cross-tenant data exfiltration.
    TenantMismatch {
        /// Expected tenant from request header
        expected: String,
        /// Actual tenant from JWT claims
        actual: String,
    },
    /// Route policy not found for the given path + method
    ///
    /// This indicates the route is not classified in the RoutePolicyStore.
    /// Per HACK-405 (fail closed), this returns 503.
    PolicyNotFound {
        /// The path that was not found
        path: String,
        /// The HTTP method
        method: String,
    },
}

impl AuthError {
    /// Returns the HTTP status code for this error.
    #[must_use]
    pub fn status_code(&self) -> u16 {
        match self {
            Self::MissingAuthorization
            | Self::InvalidBearerScheme { .. }
            | Self::TokenExpired { .. }
            | Self::TokenInvalid
            | Self::MissingTenantId
            | Self::TenantMismatch { .. } => 401,
            Self::PolicyNotFound { .. } => 503,
        }
    }

    /// Returns the error message for display.
    pub fn message(&self) -> String {
        match self {
            Self::MissingAuthorization => "Missing Authorization header".to_string(),
            Self::InvalidBearerScheme { scheme } => {
                format!("Invalid Authorization scheme: {scheme}. Only Bearer is accepted")
            }
            Self::TokenExpired { expired_at } => {
                format!("Token expired at {expired_at}")
            }
            Self::TokenInvalid => "Invalid JWT token".to_string(),
            Self::MissingTenantId => "Missing X-Tenant-ID header".to_string(),
            Self::TenantMismatch { expected, actual } => {
                format!("Tenant mismatch: expected {expected}, got {actual}")
            }
            Self::PolicyNotFound { path, method } => {
                format!("No route policy found for {method} {path}")
            }
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for AuthError {}
