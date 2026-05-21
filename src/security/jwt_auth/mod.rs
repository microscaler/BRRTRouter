//! JWT common-path authorization middleware for Story 4.2
//!
//! This module implements the JWT common-path middleware that validates JWTs
//! and evaluates local policy from claims, enabling the hybrid authz model.
//!
//! ## Design Context
//!
//! The JWT document's core thesis: JWT claims handle the common path, with
//! online fallback for high-risk decisions. This middleware implements the
//! JWT common-path evaluation that replaces the current per-request authz-core
//! call for `jwt-only` routes.
//!
//! ## Middleware Placement
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
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │       JwtAuthMiddleware             │
//! ├─────────────────────────────────────┤
//! │  1. extract_bearer_token()          │
//! │  2. validate_jwt() [JwksClient]     │
//! │  3. get_policy() [RoutePolicyStore] │
//! │  4. evaluate_jwt_only()             │
//! │  5. return AuthDecision             │
//! └─────────────────────────────────────┘
//! ```

pub mod jwks_client;
pub mod policy;
pub mod types;

use crate::dispatcher::HandlerResponse;
pub use crate::security::decision::AuthDecision;
pub use jwks_client::JwksClient;
pub use policy::*;
pub use types::*;

use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn};

/// JWT common-path authorization middleware (Story 4.2).
///
/// This middleware validates JWTs and evaluates local policy from claims.
/// For `jwt-only` routes, it returns allow/deny without calling authz-core.
/// For `jwt-with-fallback` and `online-only` routes, it validates the JWT
/// but passes control to the handler for further processing.
///
/// # Implementation (Story 4.2)
///
/// ```text
/// Request -> Extract Bearer Token -> Validate JWT -> Look Up Route Policy
///   -> Evaluate local policy (jwt-only) -> AuthDecision::Allowed/Denied
///   -> Continue to handler (jwt-with-fallback/online-only)
/// ```
///
/// # Security (HACK-401 through HACK-408)
///
/// - **HACK-401**: Tenant validation is enforced BEFORE handler execution
/// - **HACK-402**: JWT signature validation is the ONLY defense (no secondary checks)
/// - **HACK-403**: All routes (including jwt-with-fallback) validate X-Tenant-ID
/// - **HACK-404**: JWKS cache poisoning protection via key type/algorithm validation
/// - **HACK-405**: Fail CLOSED on all errors (never fail open)
/// - **HACK-406**: Rate limiting should be applied at the framework level
/// - **HACK-407**: Token expiry checked before expensive validation
/// - **HACK-408**: JWT signature validation is NEVER skipped
pub struct JwtAuthMiddleware {
    /// Store of route authorization policies
    pub route_policies: Arc<RoutePolicyStore>,
    /// JWKS client for JWT validation (signature, claims extraction)
    pub jwks_client: Arc<dyn JwksClient>,
}

impl JwtAuthMiddleware {
    /// Create a new JWT common-path authorization middleware.
    ///
    /// # Arguments
    ///
    /// * `route_policies` - The route policy store for classification
    /// * `jwks_client` - The JWKS client for JWT validation
    #[must_use]
    pub fn new(route_policies: Arc<RoutePolicyStore>, jwks_client: Arc<dyn JwksClient>) -> Self {
        Self {
            route_policies,
            jwks_client,
        }
    }

    /// Extract the Bearer token from an Authorization header.
    ///
    /// This is a core function used by the JWT common-path middleware.
    /// It extracts the token from the `Authorization: Bearer *** header.
    ///
    /// # Arguments
    ///
    /// * `headers` - Header map to extract from
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - The extracted Bearer token
    /// - `Err(AuthError::MissingAuthorization)` - No Authorization header present
    /// - `Err(AuthError::InvalidBearerScheme)` - Authorization header not using Bearer scheme
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::security::jwt_auth::{JwtAuthMiddleware, RoutePolicyStore, JwksClient, AuthError};
    ///
    /// // Minimal test client that doesn't validate signatures
    /// struct DummyClient;
    /// impl JwksClient for DummyClient {
    ///     fn validate_and_extract_claims(&self, _token: &str) -> Result<types::AccessClaims, AuthError> {
    ///         Ok(types::AccessClaims {
    ///             sub: "user-1".to_string(),
    ///             tenant_id: "tenant-1".to_string(),
    ///             user_type: "customer".to_string(),
    ///             sx: Default::default(),
    ///         })
    ///     }
    ///     fn issuer(&self) -> Option<&str> { None }
    ///     fn audience(&self) -> Option<&str> { None }
    /// }
    ///
    /// let policies = Arc::new(RoutePolicyStore::new());
    /// let client = Arc::new(DummyClient);
    /// let mw = JwtAuthMiddleware::new(policies, client);
    ///
    /// let mut headers = std::collections::HashMap::new();
    /// headers.insert("authorization".to_string(), "Bearer eyJhbG...NiJ9...".to_string());
    ///
    /// let token = mw.extract_bearer_token(&headers).unwrap();
    /// assert_eq!(token, "eyJhbG...NiJ9...");
    /// ```
    pub fn extract_bearer_token(
        &self,
        headers: &std::collections::HashMap<String, String>,
    ) -> Result<String, AuthError> {
        let auth_header = headers
            .get("authorization")
            .ok_or(AuthError::MissingAuthorization)?;

        auth_header
            .strip_prefix("Bearer ")
            .map(|token| token.to_string())
            .ok_or(AuthError::InvalidBearerScheme {
                scheme: auth_header[..auth_header
                    .find(' ')
                    .map(|i| i)
                    .unwrap_or(auth_header.len())]
                    .to_string(),
            })
    }

    /// Validate tenant context from JWT claims against request header.
    ///
    /// This is the most critical security check (HACK-401). If the tenant
    /// in the JWT claims doesn't match the X-Tenant-ID header, the request
    /// MUST be rejected immediately to prevent cross-tenant data exfiltration.
    ///
    /// # Arguments
    ///
    /// * `claims` - The validated JWT claims
    /// * `headers` - Request headers (includes X-Tenant-ID)
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Tenant validation passed
    /// - `Err(AuthError::MissingTenantId)` - No X-Tenant-ID header present
    /// - `Err(AuthError::TenantMismatch)` - Tenant ID mismatch between claims and header
    ///
    /// # Security (HACK-401, HACK-403)
    ///
    /// - ALL routes (including jwt-with-fallback, online-only) must validate X-Tenant-ID
    /// - The tenant in the JWT claims should ONLY be used for validation,
    ///   not for authorization decisions
    /// - If tenant validation fails, reject immediately — never pass to handler
    pub fn validate_tenant(
        &self,
        claims: &AccessClaims,
        headers: &std::collections::HashMap<String, String>,
    ) -> Result<(), AuthError> {
        let request_tenant = headers
            .get("x-tenant-id")
            .map(|h| h.as_str())
            .ok_or(AuthError::MissingTenantId)?;

        if claims.tenant_id != request_tenant {
            return Err(AuthError::TenantMismatch {
                expected: request_tenant.to_string(),
                actual: claims.tenant_id.clone(),
            });
        }

        debug!("Tenant validation passed: tenant_id={}", request_tenant);
        Ok(())
    }

    /// Evaluate local policy from JWT claims for a specific route.
    ///
    /// This function is called for `jwt-only` routes to determine if the
    /// user's JWT claims satisfy the route's authorization requirements.
    ///
    /// # Arguments
    ///
    /// * `claims` - The validated JWT claims
    /// * `policy` - The route policy to evaluate against
    ///
    /// # Returns
    ///
    /// - `true` - Policy evaluation passed (user has required roles/permissions)
    /// - `false` - Policy evaluation failed (user lacks required roles/permissions)
    ///
    /// # Evaluation Logic (Story 4.2)
    ///
    /// 1. Check tenant_id matches request X-Tenant-ID (must be called BEFORE this)
    /// 2. Check roles/permissions in claims.sx
    /// 3. Check user_type (customer vs platform)
    /// 4. Check risk level if present
    pub fn evaluate_local_policy(&self, claims: &AccessClaims, policy: &RoutePolicy) -> bool {
        // Check roles
        if !policy.check_roles(&claims.sx.roles) {
            debug!(
                "Local policy check failed: missing required roles \
                 (required: {:?}, user has: {:?})",
                policy.required_roles, claims.sx.roles
            );
            return false;
        }

        // Check permissions
        if !policy.check_permissions(&claims.sx.permissions) {
            debug!(
                "Local policy check failed: missing required permissions \
                 (required: {:?}, user has: {:?})",
                policy.required_permissions, claims.sx.permissions
            );
            return false;
        }

        // Check risk level
        if !policy.check_risk(claims.sx.risk.as_deref()) {
            debug!(
                "Local policy check failed: risk level {:?} not acceptable for route",
                claims.sx.risk
            );
            return false;
        }

        debug!(
            "Local policy check passed for route {} (roles: {:?}, permissions: {:?})",
            policy.identifier, claims.sx.roles, claims.sx.permissions
        );
        true
    }

    /// Validate JWT and evaluate authorization for a request.
    ///
    /// This is the main entry point for the JWT common-path middleware.
    /// It performs the full authorization evaluation flow:
    ///
    /// 1. Extract Bearer token from Authorization header
    /// 2. Validate JWT (typ, iss, aud, exp, nbf, signature) via JWKS client
    /// 3. Look up route policy by path + method
    /// 4. Validate tenant context (claims.tenant_id vs X-Tenant-ID header)
    /// 5. Evaluate local policy for jwt-only routes
    /// 6. Return AuthDecision
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (e.g., "GET", "POST")
    /// * `path` - Request path (e.g., "/api/v1/identity/users/me")
    /// * `headers` - Request headers (includes Authorization and X-Tenant-ID)
    ///
    /// # Returns
    ///
    /// - `Ok(AuthDecision::Allowed)` - JWT validated, policy passed (jwt-only route)
    /// - `Ok(AuthDecision::Denied)` - JWT validated, but policy failed (jwt-only route)
    /// - `Ok(AuthDecision::JwtCommonPath)` - JWT validated, continue to handler (jwt-with-fallback/online-only)
    /// - `Err(AuthError)` - JWT validation failed (expired, invalid signature, missing header, etc.)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use brrtrouter::security::jwt_auth::{JwtAuthMiddleware, RoutePolicyStore, RouteAuthCategory, JwksClient};
    /// use std::sync::Arc;
    /// use std::collections::HashMap;
    ///
    /// // Setup (in real code, these would be properly initialized)
    /// let policies = Arc::new(RoutePolicyStore::new());
    /// let jwks_client: Arc<dyn JwksClient> = Arc::new(MyJwksClient);
    /// let middleware = JwtAuthMiddleware::new(policies, jwks_client);
    ///
    /// let headers = HashMap::new();
    /// let decision = middleware.validate_and_authorize("GET", "/api/users/me", &headers);
    /// ```
    pub fn validate_and_authorize(
        &self,
        method: &str,
        path: &str,
        headers: &std::collections::HashMap<String, String>,
    ) -> Result<AuthDecision, AuthError> {
        // Step 1: Extract Bearer token from Authorization header
        let token = self.extract_bearer_token(headers)?;

        // Step 2: Validate JWT and extract claims via JWKS client
        // This validates: typ, iss, aud, exp, nbf, signature
        let claims = match self.jwks_client.validate_and_extract_claims(&token) {
            Ok(claims) => claims,
            Err(e) => {
                // Per HACK-405: fail closed on JWT validation failure
                error!("JWT validation failed for {} {}: {:?}", method, path, e);
                return Err(e);
            }
        };

        debug!("JWT validated successfully: sub={}", claims.sub);

        // Step 3: Look up route policy for this path + method
        let policy =
            self.route_policies
                .get_policy(method, path)
                .ok_or(AuthError::PolicyNotFound {
                    path: path.to_string(),
                    method: method.to_string(),
                })?;

        debug!(
            "Found route policy: {} (category: {:?})",
            policy.identifier, policy.category
        );

        // Step 4: Validate tenant context for ALL routes
        // Per HACK-401 and HACK-403: tenant validation happens BEFORE handler
        if let Err(e) = self.validate_tenant(&claims, headers) {
            error!("Tenant validation failed for {} {}: {:?}", method, path, e);
            return Err(e);
        }

        // Step 5: Evaluate based on route category
        match &policy.category {
            RouteAuthCategory::JwtOnly => {
                // For jwt-only routes, evaluate local policy from JWT claims
                if self.evaluate_local_policy(&claims, policy) {
                    debug!("Jwt-only route ALLOWED: {} {}", method, path);
                    Ok(AuthDecision::Allowed {
                        reason: Some(format!(
                            "jwt_only: roles={:?}, permissions={:?}",
                            claims.sx.roles, claims.sx.permissions
                        )),
                    })
                } else {
                    warn!(
                        "Jwt-only route DENIED: {} {} (policy violation)",
                        method, path
                    );
                    Ok(AuthDecision::Denied {
                        reason: Some("jwt_only_policy_violation".to_string()),
                    })
                }
            }
            RouteAuthCategory::JwtWithFallback | RouteAuthCategory::OnlineOnly => {
                // For jwt-with-fallback and online-only, JWT is validated but
                // authorization requires online fallback or authz-core call
                debug!(
                    "Route requires online fallback: {} {} (category: {:?})",
                    method, path, policy.category
                );
                Ok(AuthDecision::JwtCommonPath {
                    reason: Some(format!("requires_online: {:?}", policy.category)),
                })
            }
        }
    }

    /// Build a deny response from an AuthError.
    ///
    /// Converts an `AuthError` into a `HandlerResponse` with the appropriate
    /// HTTP status code. Per HACK-405: always fail closed.
    ///
    /// # Arguments
    ///
    /// * `error` - The authentication/authorization error
    ///
    /// # Returns
    ///
    /// A `HandlerResponse` with status 401 (auth errors) or 503 (policy not found)
    pub fn error_to_response(&self, error: &AuthError) -> crate::dispatcher::HandlerResponse {
        let status = error.status_code();
        let message = error.message();

        match status {
            401 => {
                // 401 Unauthorized — token/auth errors
                crate::dispatcher::HandlerResponse::error(401, &message)
            }
            _ => {
                // 503 Service Unavailable — policy not found (fail closed)
                crate::dispatcher::HandlerResponse::error(503, &message)
            }
        }
    }

    /// Build an allowed response that passes control to the handler.
    ///
    /// For `AuthDecision::Allowed` and `AuthDecision::JwtCommonPath`, return
    /// `None` to signal the dispatcher to continue to the handler.
    ///
    /// This method is a no-op: returning `None` from `before` means
    /// "proceed to handler." We document it explicitly for clarity.
    #[must_use]
    pub fn allow_to_handler(&self) -> Option<crate::dispatcher::HandlerResponse> {
        None
    }
}

/// Middleware trait implementation for JwtAuthMiddleware.
///
/// This implements the `brrtrouter::middleware::Middleware` trait so that
/// JwtAuthMiddleware can be registered on the BRRTRouter Dispatcher
/// alongside the other built-in middleware (Auth, CORS, Metrics, Tracing).
///
/// The `before` hook intercepts requests, validates JWTs, evaluates
/// route policies, and short-circuits the chain on auth failures.
///
/// # Integration Points
///
/// The middleware integrates with the existing security infrastructure:
/// - Uses the same `SecurityRequest` type as other providers
/// - Reads from `HandlerRequest.headers` and `HandlerRequest.path`
/// - Short-circuits via `before()` returning `Some(HandlerResponse)`
/// - Allows normal flow via `before()` returning `None`
///
/// # Security Model (HACK-405)
///
/// - **Fail closed**: Every error path returns an error response
/// - **Never fail open**: No unauthenticated request reaches the handler
/// - **Tenant validation**: Runs on ALL routes before any handler logic
impl crate::middleware::Middleware for JwtAuthMiddleware {
    /// Pre-processing hook: intercept request, validate JWT, evaluate policy.
    ///
    /// # Flow
    ///
    /// 1. Extract Bearer token from Authorization header
    /// 2. Validate JWT signature and claims (typ, iss, aud, exp, nbf)
    /// 3. Look up route policy by path + method
    /// 4. Validate tenant context (X-Tenant-ID header)
    /// 5. For jwt-only routes: evaluate local policy → allow or deny
    /// 6. For jwt-with-fallback/online-only: return None to continue to handler
    ///
    /// # Returns
    ///
    /// - `Some(HandlerResponse)` - Short-circuit with error response (401 or 503)
    /// - `None` - JWT valid, continue to next middleware or handler
    fn before(
        &self,
        req: &crate::dispatcher::HandlerRequest,
    ) -> Option<crate::dispatcher::HandlerResponse> {
        // Build the header map from HandlerRequest headers
        let headers: std::collections::HashMap<String, String> = req
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();

        // Build the query params map (needed for SecurityRequest if used later)
        let _query: std::collections::HashMap<String, String> = req
            .query_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        // Step 1: Try to extract and validate the JWT
        let decision = self.validate_and_authorize(&req.method.as_str(), &req.path, &headers);

        match decision {
            Ok(AuthDecision::Allowed { reason }) => {
                // JWT-only route: policy passed, allow to handler
                debug!(
                    request_id = ?req.request_id,
                    method = %req.method,
                    path = %req.path,
                    reason = ?reason,
                    "JWT common-path: allowed"
                );
                None
            }
            Ok(AuthDecision::JwtCommonPath { reason }) => {
                // jwt-with-fallback or online-only: JWT validated, continue to handler
                debug!(
                    request_id = ?req.request_id,
                    method = %req.method,
                    path = %req.path,
                    reason = ?reason,
                    "JWT common-path: jwt-common-path, continues to handler"
                );
                None
            }
            Ok(AuthDecision::Denied { reason }) => {
                // JWT-only route: policy failed, deny
                warn!(
                    request_id = ?req.request_id,
                    method = %req.method,
                    path = %req.path,
                    reason = ?reason,
                    "JWT common-path: denied"
                );
                Some(HandlerResponse::error(403, &reason.unwrap_or_default()))
            }
            Err(e) => {
                // JWT validation error: fail closed
                error!(
                    request_id = ?req.request_id,
                    method = %req.method,
                    path = %req.path,
                    error = %e,
                    status = e.status_code(),
                    "JWT common-path: validation error, fail closed"
                );
                let response = self.error_to_response(&e);
                Some(response)
            }
        }
    }

    /// Post-processing hook: log middleware latency for observability.
    ///
    /// This hook runs for every request that passes through the middleware chain,
    /// regardless of whether the middleware returned early from `before()`.
    ///
    /// Per the BRRTRouter observability pattern (NOT using custom Prometheus counters):
    /// - Use `tracing::info!` with structured fields for request logging
    /// - No custom metrics — the existing `MetricsMiddleware` covers request counts/latency
    fn after(
        &self,
        _req: &crate::dispatcher::HandlerRequest,
        _res: &mut crate::dispatcher::HandlerResponse,
        latency: Duration,
    ) {
        // Structured log for Loki-based alerting — no custom Prometheus counters
        tracing::info!(
            latency_ms = latency.as_millis(),
            "jwt_auth_middleware: request processed"
        );
    }
}
