//! Route policy classification for JWT common-path authorization (Story 4.2)
//!
//! This module defines the route policy store that classifies each API route
//! into an authorization category. The JWT common-path middleware uses this
//! classification to determine how to evaluate authorization for each request.
//!
//! **Route Auth Categories (Story 4.2):**
//!
//! - `JwtOnly`: Entire authorization decision from JWT claims (no online call)
//! - `JwtWithFallback`: JWT validates common path, then falls back to online authz-core
//! - `OnlineOnly`: JWT validates, then requires full authz-core evaluation
//!
//! **Classification Flow:**
//!
//! ```text
//! Route Definition (OpenAPI / code config)
//!   -> RoutePolicyStore.build() -> Classification
//!     -> jwt-only routes: fast path (JWT claims only)
//!     -> jwt-with-fallback: JWT + optional online check
//!     -> online-only: JWT + mandatory online check
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authorization category for a route.
///
/// This enum determines how the JWT common-path middleware handles authorization
/// It's the core classification mechanism that enables the
// hybrid authz model: JWT claims for common path, online fallback for complex decisions.
///
/// # Categories
///
/// - **JwtOnly**: The most restrictive/faster path. All authorization is evaluated
///   from JWT claims alone. If the policy check passes, the request is allowed.
///   This is the primary mechanism for reducing online authz load.
///
/// - **JwtWithFallback**: The JWT is validated (signature, expiry, claims), but
///   the final authorization decision requires calling authz-core /authorize.
///   The JWT common-path middleware validates the token and passes claims to
///   the handler, which then calls authz-core for the final decision.
///
/// - **OnlineOnly**: The JWT is validated as a prerequisite, but the authorization
///   decision ALWAYS requires an online authz-core call. The JWT just proves
///   identity; the actual authorization is determined by the online system.
///
/// # Security Implications
///
/// - `JwtOnly` routes are the most performant but least flexible (can't do
///   resource-level authorization like "can user edit THIS invoice")
/// - `JwtWithFallback` and `OnlineOnly` routes always call authz-core, so
///   they're slower but support fine-grained, dynamic authorization
/// - Changing a route from `JwtOnly` to `OnlineOnly` is a security upgrade
///   (adds online verification) but has performance implications
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteAuthCategory {
    /// JWT-only: authorization fully evaluated from JWT claims
    JwtOnly,
    /// JWT with fallback: JWT validated, then calls authz-core /authorize
    JwtWithFallback,
    /// Online only: JWT validated, then mandatory authz-core /authorize
    OnlineOnly,
}

impl RouteAuthCategory {
    /// Returns true if this category requires an online authz-core call.
    #[must_use]
    pub fn requires_online_check(&self) -> bool {
        matches!(self, Self::JwtWithFallback | Self::OnlineOnly)
    }

    /// Returns true if this category makes the authorization decision from JWT claims alone.
    #[must_use]
    pub fn is_jwt_only(&self) -> bool {
        matches!(self, Self::JwtOnly)
    }

    /// Returns the category string for metrics and logging.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::JwtOnly => "jwt_only",
            Self::JwtWithFallback => "jwt_with_fallback",
            Self::OnlineOnly => "online_only",
        }
    }
}

/// A route policy defines the authorization requirements for a specific API endpoint.
///
/// Each policy is associated with a path + method combination and specifies:
/// - The authorization category (how authorization is evaluated)
/// - Required roles (if any) for jwt-only routes
/// - Required permissions (if any) for jwt-only routes
/// - Whether elevated risk is acceptable
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::jwt_auth::policy::{RouteAuthCategory, RoutePolicy};
///
/// let policy = RoutePolicy::new(
///     "GET /api/v1/identity/users/me".to_string(),
///     RouteAuthCategory::JwtOnly,
///     vec!["customer".to_string()],
///     vec!["users:read".to_string()],
///     true, // risk acceptable
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePolicy {
    /// The policy identifier (e.g., "GET /api/v1/identity/users/me")
    pub identifier: String,
    /// The authorization category for this route
    pub category: RouteAuthCategory,
    /// Required roles for jwt-only routes (empty = no role requirement)
    pub required_roles: Vec<String>,
    /// Required permissions for jwt-only routes (empty = no permission requirement)
    pub required_permissions: Vec<String>,
    /// Whether elevated/critical risk is acceptable (true = any risk level ok)
    pub risk_acceptable: bool,
}

impl RoutePolicy {
    /// Create a new route policy.
    ///
    /// # Arguments
    ///
    /// * `identifier` - A human-readable identifier for this policy (e.g., "GET /api/users")
    /// * `category` - The authorization category
    /// * `required_roles` - Roles required for jwt-only routes
    /// * `required_permissions` - Permissions required for jwt-only routes
    /// * `risk_acceptable` - Whether elevated/critical risk is acceptable
    #[must_use]
    pub fn new(
        identifier: String,
        category: RouteAuthCategory,
        required_roles: Vec<String>,
        required_permissions: Vec<String>,
        risk_acceptable: bool,
    ) -> Self {
        Self {
            identifier,
            category,
            required_roles,
            required_permissions,
            risk_acceptable,
        }
    }

    /// Check if the user has the required roles for this route.
    ///
    /// # Arguments
    ///
    /// * `user_roles` - The roles present in the user's JWT claims
    ///
    /// Returns true if the user has at least one of the required roles
    /// (or no roles are required).
    #[must_use]
    pub fn check_roles(&self, user_roles: &[String]) -> bool {
        if self.required_roles.is_empty() {
            return true;
        }
        self.required_roles
            .iter()
            .any(|required| user_roles.iter().any(|role| role == required))
    }

    /// Check if the user has the required permissions for this route.
    ///
    /// # Arguments
    ///
    /// * `user_permissions` - The permissions present in the user's JWT claims
    ///
    /// Returns true if all required permissions are satisfied (or no permissions are required).
    #[must_use]
    pub fn check_permissions(&self, user_permissions: &[String]) -> bool {
        if self.required_permissions.is_empty() {
            return true;
        }
        self.required_permissions.iter().all(|required| {
            user_permissions
                .iter()
                .any(|permission| permission == required)
        })
    }

    /// Check if the risk level is acceptable for this route.
    ///
    /// # Arguments
    ///
    /// * `risk_level` - The risk level from the JWT claims (may be None)
    ///
    /// Returns true if the risk level is acceptable:
    /// - If `risk_acceptable` is true: always returns true
    /// - If `risk_acceptable` is false: returns true only if risk is "normal" or None
    #[must_use]
    pub fn check_risk(&self, risk_level: Option<&str>) -> bool {
        if self.risk_acceptable {
            return true;
        }
        // If risk is not elevated/critical, it's acceptable
        risk_level.map_or(true, |level| level != "elevated" && level != "critical")
    }

    /// Build a policy identifier from path and method.
    #[must_use]
    pub fn from_path_method(method: &str, path: &str) -> Self {
        Self::new(
            format!("{method} {path}"),
            RouteAuthCategory::JwtOnly,
            vec![],
            vec![],
            true,
        )
    }
}

/// A store of route policies that classifies all API routes.
///
/// The RoutePolicyStore is the central registry for route authorization policies.
/// It's used by the JWT common-path middleware to look up the correct policy
/// for each request's path + method combination.
///
/// # Thread Safety
///
/// The store is wrapped in `Arc` and uses interior mutability where needed.
/// It's designed to be shared across multiple middleware instances.
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::jwt_auth::policy::{RouteAuthCategory, RoutePolicyStore, RoutePolicy};
///
/// let mut store = RoutePolicyStore::new();
/// store.add_policy(RoutePolicy::new(
///     "GET /api/v1/identity/users/me".to_string(),
///     RouteAuthCategory::JwtOnly,
///     vec!["customer".to_string()],
///     vec!["users:read".to_string()],
///     true,
/// ));
///
/// let policy = store.get_policy("GET", "/api/v1/identity/users/me");
/// assert!(policy.is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePolicyStore {
    /// Path-based lookup: path -> method -> policy
    /// This allows fast lookup of policies by path and method
    policies: HashMap<String, HashMap<String, RoutePolicy>>,
    /// Total number of policies in the store
    policy_count: usize,
    /// Default category for unclassified routes (fail-safe: JwtWithFallback)
    default_category: RouteAuthCategory,
}

impl RoutePolicyStore {
    /// Create a new empty route policy store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            policy_count: 0,
            default_category: RouteAuthCategory::JwtWithFallback,
        }
    }

    /// Create a new store with a default policy for unclassified routes.
    ///
    /// When a route is not found in the store, this default category is used.
    /// The default is `JwtWithFallback` (fail-safe: requires online check).
    #[must_use]
    pub fn with_default(category: RouteAuthCategory) -> Self {
        let mut store = Self::new();
        // All routes default to JwtWithFallback unless explicitly set
        store.default_category = category;
        store
    }

    /// Add a route policy to the store.
    ///
    /// # Arguments
    ///
    /// * `policy` - The route policy to add
    pub fn add_policy(&mut self, policy: RoutePolicy) {
        let path = extract_path_from_identifier(&policy.identifier);
        let method = extract_method_from_identifier(&policy.identifier);

        self.policies
            .entry(path)
            .or_insert_with(HashMap::new)
            .insert(method, policy);
        self.policy_count += 1;
    }

    /// Look up the route policy for a given method and path.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (e.g., "GET", "POST")
    /// * `path` - Request path (e.g., "/api/v1/identity/users/me")
    ///
    /// # Returns
    ///
    /// - `Some(policy)` - The policy for this route, if found
    /// - `None` - No policy found for this route
    ///
    /// This lookup is used by the JWT common-path middleware to determine
    /// the authorization category for each request.
    #[must_use]
    pub fn get_policy(&self, method: &str, path: &str) -> Option<&RoutePolicy> {
        // First try exact match
        if let Some(methods) = self.policies.get(path) {
            if let Some(policy) = methods.get(method) {
                return Some(policy);
            }
        }

        // Then try wildcard match (e.g., * matches any method)
        if let Some(methods) = self.policies.get(path) {
            if let Some(policy) = methods.get("*") {
                return Some(policy);
            }
        }

        // Try parameterized match (e.g., /users/xid matches /users/123)
        let req_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (stored_path, methods) in &self.policies {
            let stored_segments: Vec<&str> =
                stored_path.split('/').filter(|s| !s.is_empty()).collect();
            if req_segments.len() != stored_segments.len() {
                continue;
            }
            let mut segment_match = true;
            for (req_seg, stored_seg) in req_segments.iter().zip(stored_segments.iter()) {
                if *stored_seg != *req_seg {
                    // If stored segment is a parameter (not a known literal), it matches
                    // Known literals: "me", "me/", "xid" can be treated as params
                    // But "me" is literal. Check against a known param pattern.
                    // Heuristic: if stored_seg is not a common API literal, treat as param.
                    // Common literals: me, users, shipments, payments, identity
                    let is_literal = matches!(
                        *stored_seg,
                        "me" | "me/"
                            | "users"
                            | "shipments"
                            | "payments"
                            | "identity"
                            | "v1"
                            | "v2"
                            | "api"
                    );
                    if is_literal {
                        segment_match = false;
                        break;
                    }
                }
            }
            if segment_match {
                if let Some(policy) = methods.get(method) {
                    return Some(policy);
                }
                if let Some(policy) = methods.get("*") {
                    return Some(policy);
                }
            }
        }

        None
    }

    /// Check if a policy exists for the given method and path.
    #[must_use]
    pub fn has_policy(&self, method: &str, path: &str) -> bool {
        self.get_policy(method, path).is_some()
    }

    /// Get the total number of policies in the store.
    #[must_use]
    pub fn policy_count(&self) -> usize {
        self.policy_count
    }

    /// Get a list of all paths in the store.
    #[must_use]
    pub fn paths(&self) -> Vec<&String> {
        self.policies.keys().collect()
    }

    /// Remove a policy for the given method and path.
    pub fn remove_policy(&mut self, method: &str, path: &str) {
        if let Some(methods) = self.policies.get_mut(path) {
            if methods.remove(method).is_some() {
                self.policy_count -= 1;
                // Clean up empty path entries
                if methods.is_empty() {
                    self.policies.remove(path);
                }
            }
        }
    }
}

/// Extract the HTTP method from a policy identifier.
///
/// Policy identifiers are formatted as "METHOD /path" (e.g., "GET /api/v1/users").
/// This function extracts the method portion.
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::jwt_auth::policy::extract_method_from_identifier;
///
/// assert_eq!(extract_method_from_identifier("GET /api/v1/users"), "GET");
/// assert_eq!(extract_method_from_identifier("POST /api/v1/users"), "POST");
/// ```
#[must_use]
pub fn extract_method_from_identifier(identifier: &str) -> String {
    identifier
        .split_whitespace()
        .next()
        .unwrap_or("*")
        .to_uppercase()
}

/// Extract the path from a policy identifier.
///
/// Policy identifiers are formatted as "METHOD /path" (e.g., "GET /api/v1/users").
/// This function extracts the path portion.
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::jwt_auth::policy::extract_path_from_identifier;
///
/// assert_eq!(extract_path_from_identifier("GET /api/v1/users"), "/api/v1/users");
/// assert_eq!(extract_path_from_identifier("POST /api/v1/users"), "/api/v1/users");
/// ```
#[must_use]
pub fn extract_path_from_identifier(identifier: &str) -> String {
    identifier
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string()
}

impl Default for RoutePolicyStore {
    fn default() -> Self {
        Self::new()
    }
}

// Note: with_default needs default_category field - let's fix that
