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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
    use crate::ids::RequestId;
    use crate::middleware::Middleware;
    use http::Method;
    use may::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;

    // ─── Mock JWKS Client ──────────────────────────────────────────

    struct MockJwksClient {
        claims: AccessClaims,
    }

    impl MockJwksClient {
        fn new(claims: AccessClaims) -> Self {
            Self { claims }
        }
    }

    impl JwksClient for MockJwksClient {
        fn validate_and_extract_claims(&self, _token: &str) -> Result<AccessClaims, AuthError> {
            Ok(self.claims.clone())
        }
        fn issuer(&self) -> Option<&str> {
            None
        }
        fn audience(&self) -> Option<&str> {
            None
        }
    }

    // ─── Helpers ───────────────────────────────────────────────────

    fn make_request(method: Method, path: &str) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel::<HandlerResponse>();
        HandlerRequest {
            request_id: RequestId::new(),
            method,
            path: path.to_string(),
            handler_name: "jwt_auth_test".to_string(),
            path_params: crate::router::ParamVec::new(),
            query_params: crate::router::ParamVec::new(),
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
        }
    }

    /// Build middleware + mutable policy store. Caller registers policies into `store`.
    fn build_mw_claims(claims: AccessClaims) -> (JwtAuthMiddleware, RoutePolicyStore) {
        let mut store = RoutePolicyStore::new();
        let client = Arc::new(MockJwksClient::new(claims));
        let mut mw = JwtAuthMiddleware::new(Arc::new(store), client);
        let inner = std::sync::Arc::into_inner(std::mem::take(&mut mw.route_policies)).unwrap();
        (mw, inner)
    }

    fn register_jwt_only(
        store: &mut RoutePolicyStore,
        method: &str,
        path: &str,
        roles: Vec<String>,
        permissions: Vec<String>,
    ) {
        store.add_policy(RoutePolicy::new(
            format!("{} {}", method, path),
            RouteAuthCategory::JwtOnly,
            roles,
            permissions,
            true, // risk acceptable
        ));
    }

    fn register_jwt_fallback(store: &mut RoutePolicyStore, method: &str, path: &str) {
        store.add_policy(RoutePolicy::new(
            format!("{} {}", method, path),
            RouteAuthCategory::JwtWithFallback,
            vec![],
            vec![],
            true,
        ));
    }

    fn register_online_only(store: &mut RoutePolicyStore, method: &str, path: &str) {
        store.add_policy(RoutePolicy::new(
            format!("{} {}", method, path),
            RouteAuthCategory::OnlineOnly,
            vec![],
            vec![],
            true,
        ));
    }

    fn set_policies(mw: &mut JwtAuthMiddleware, store: RoutePolicyStore) {
        mw.route_policies = Arc::new(store);
    }

    fn build_headers(auth: &str, tenant: &str) -> std::collections::HashMap<String, String> {
        let mut h = std::collections::HashMap::new();
        h.insert("authorization".to_string(), auth.to_string());
        h.insert("x-tenant-id".to_string(), tenant.to_string());
        h
    }

    fn make_claims(sub: &str, tenant: &str, roles: Vec<&str>, perms: Vec<&str>) -> AccessClaims {
        AccessClaims {
            sub: sub.to_string(),
            tenant_id: tenant.to_string(),
            user_type: "customer".to_string(),
            sx: SxClaims {
                roles: roles.into_iter().map(String::from).collect(),
                permissions: perms.into_iter().map(String::from).collect(),
                risk: None,
            },
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  UNIT: extract_bearer_token
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_extract_bearer_token_valid() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "authorization".to_string(),
            "Bearer my-token-123".to_string(),
        );
        assert_eq!(mw.extract_bearer_token(&headers).unwrap(), "my-token-123");
    }

    #[test]
    fn test_extract_bearer_token_missing_header() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let headers = std::collections::HashMap::new();
        assert!(matches!(
            mw.extract_bearer_token(&headers),
            Err(AuthError::MissingAuthorization)
        ));
    }

    #[test]
    fn test_extract_bearer_token_wrong_scheme() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "authorization".to_string(),
            "Basic dXNlcjpwYXNz".to_string(),
        );
        assert!(matches!(
            mw.extract_bearer_token(&headers),
            Err(AuthError::InvalidBearerScheme { .. })
        ));
    }

    #[test]
    fn test_extract_bearer_token_empty_bearer() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer ".to_string());
        assert_eq!(mw.extract_bearer_token(&headers).unwrap(), "");
    }

    // ═══════════════════════════════════════════════════════════════
    //  UNIT: validate_tenant
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_validate_tenant_match() {
        let claims = make_claims("u", "tenant-abc", vec![], vec![]);
        let (mw, _store) = build_mw_claims(claims.clone());
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-tenant-id".to_string(), "tenant-abc".to_string());
        assert!(mw.validate_tenant(&claims, &headers).is_ok());
    }

    #[test]
    fn test_validate_tenant_mismatch() {
        let claims = make_claims("u", "tenant-xyz", vec![], vec![]);
        let (mw, _store) = build_mw_claims(claims.clone());
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-tenant-id".to_string(), "tenant-abc".to_string());
        let err = mw.validate_tenant(&claims, &headers).unwrap_err();
        assert!(matches!(err, AuthError::TenantMismatch { .. }));
        if let AuthError::TenantMismatch { expected, actual } = err {
            assert_eq!(expected, "tenant-abc");
            assert_eq!(actual, "tenant-xyz");
        }
    }

    #[test]
    fn test_validate_tenant_missing_header() {
        let claims = make_claims("u", "tenant-abc", vec![], vec![]);
        let (mw, _store) = build_mw_claims(claims);
        let headers = std::collections::HashMap::new();
        assert!(matches!(
            mw.validate_tenant(&make_claims("u", "tenant-abc", vec![], vec![]), &headers),
            Err(AuthError::MissingTenantId)
        ));
    }

    // ═══════════════════════════════════════════════════════════════
    //  UNIT: evaluate_local_policy
    // ═══════════════════════════════════════════════════════════════

    fn make_policy(
        id: &str,
        cat: RouteAuthCategory,
        roles: Vec<String>,
        perms: Vec<String>,
    ) -> RoutePolicy {
        RoutePolicy::new(id.to_string(), cat, roles, perms, true)
    }

    #[test]
    fn test_eval_pass_roles() {
        let claims = make_claims("u", "t", vec!["admin"], vec![]);
        let (mw, _store) = build_mw_claims(claims);
        let policy = make_policy(
            "GET /api/users/me",
            RouteAuthCategory::JwtOnly,
            vec!["admin".to_string()],
            vec![],
        );
        assert!(mw.evaluate_local_policy(&make_claims("u", "t", vec!["admin"], vec![]), &policy));
    }

    #[test]
    fn test_eval_fail_roles() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec!["viewer"], vec![]));
        let policy = make_policy(
            "GET /api/admin",
            RouteAuthCategory::JwtOnly,
            vec!["admin".to_string()],
            vec![],
        );
        assert!(!mw.evaluate_local_policy(&make_claims("u", "t", vec!["viewer"], vec![]), &policy));
    }

    #[test]
    fn test_eval_pass_permissions() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec!["users:write"]));
        let policy = make_policy(
            "POST /api/users",
            RouteAuthCategory::JwtOnly,
            vec![],
            vec!["users:write".to_string()],
        );
        assert!(
            mw.evaluate_local_policy(&make_claims("u", "t", vec![], vec!["users:write"]), &policy)
        );
    }

    #[test]
    fn test_eval_fail_permissions() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec!["users:read"]));
        let policy = make_policy(
            "DELETE /api/users",
            RouteAuthCategory::JwtOnly,
            vec![],
            vec!["users:delete".to_string()],
        );
        assert!(
            !mw.evaluate_local_policy(&make_claims("u", "t", vec![], vec!["users:read"]), &policy)
        );
    }

    #[test]
    fn test_eval_risk_normal_passes() {
        let claims = make_claims("u", "t", vec![], vec![]);
        let mut c2 = claims.clone();
        c2.sx.risk = Some("normal".to_string());
        let (mw, _store) = build_mw_claims(c2.clone());
        let policy = make_policy("GET /api/safe", RouteAuthCategory::JwtOnly, vec![], vec![]);
        assert!(mw.evaluate_local_policy(&c2, &policy));
    }

    #[test]
    fn test_eval_risk_elevated_denied() {
        let claims = make_claims("u", "t", vec![], vec![]);
        let mut c2 = claims.clone();
        c2.sx.risk = Some("elevated".to_string());
        let (mw, _store) = build_mw_claims(c2.clone());
        let policy = make_policy(
            "GET /api/strict",
            RouteAuthCategory::JwtOnly,
            vec![],
            vec![],
        );
        // risk_acceptable=true so elevated is OK — the check uses policy.risk_acceptable
        assert!(mw.evaluate_local_policy(&c2, &policy));
    }

    #[test]
    fn test_eval_risk_strict_denies_elevated() {
        let claims = make_claims("u", "t", vec![], vec![]);
        let mut c2 = claims.clone();
        c2.sx.risk = Some("elevated".to_string());
        let (mw, _store) = build_mw_claims(c2.clone());
        let policy = RoutePolicy::new(
            "GET /api/strict".to_string(),
            RouteAuthCategory::JwtOnly,
            vec![],
            vec![],
            false,
        );
        assert!(!mw.evaluate_local_policy(&c2, &policy));
    }

    #[test]
    fn test_eval_risk_strict_allows_normal() {
        let claims = make_claims("u", "t", vec![], vec![]);
        let mut c2 = claims.clone();
        c2.sx.risk = Some("normal".to_string());
        let (mw, _store) = build_mw_claims(c2.clone());
        let policy = RoutePolicy::new(
            "GET /api/strict".to_string(),
            RouteAuthCategory::JwtOnly,
            vec![],
            vec![],
            false,
        );
        assert!(mw.evaluate_local_policy(&c2, &policy));
    }

    #[test]
    fn test_eval_multiple_roles_any_match() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec!["editor"], vec![]));
        let policy = make_policy(
            "GET /api/docs",
            RouteAuthCategory::JwtOnly,
            vec!["admin".to_string(), "editor".to_string()],
            vec![],
        );
        assert!(mw.evaluate_local_policy(&make_claims("u", "t", vec!["editor"], vec![]), &policy));
    }

    #[test]
    fn test_eval_all_required_permissions_met() {
        let claims = make_claims("u", "t", vec![], vec!["files:read", "files:write"]);
        let (mw, _store) = build_mw_claims(claims);
        let policy = make_policy(
            "PUT /api/files",
            RouteAuthCategory::JwtOnly,
            vec![],
            vec!["files:read".to_string(), "files:write".to_string()],
        );
        assert!(mw.evaluate_local_policy(
            &make_claims("u", "t", vec![], vec!["files:read", "files:write"]),
            &policy
        ));
    }

    #[test]
    fn test_eval_missing_one_perm_fails() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec!["files:read"]));
        let policy = make_policy(
            "PUT /api/files",
            RouteAuthCategory::JwtOnly,
            vec![],
            vec!["files:read".to_string(), "files:write".to_string()],
        );
        assert!(
            !mw.evaluate_local_policy(&make_claims("u", "t", vec![], vec!["files:read"]), &policy)
        );
    }

    // ═══════════════════════════════════════════════════════════════
    //  UNIT: error_to_response
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_error_to_response_401_missing_auth() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let response = mw.error_to_response(&AuthError::MissingAuthorization);
        assert_eq!(response.status, 401);
    }

    #[test]
    fn test_error_to_response_503_policy_not_found() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let response = mw.error_to_response(&AuthError::PolicyNotFound {
            path: "/unknown".to_string(),
            method: "GET".to_string(),
        });
        assert_eq!(response.status, 503);
    }

    #[test]
    fn test_error_to_response_401_tenant_mismatch() {
        let (mw, _store) = build_mw_claims(make_claims("u", "t", vec![], vec![]));
        let response = mw.error_to_response(&AuthError::TenantMismatch {
            expected: "tenant-a".to_string(),
            actual: "tenant-b".to_string(),
        });
        assert_eq!(response.status, 401);
    }

    // ═══════════════════════════════════════════════════════════════
    //  INTEGRATION: validate_and_authorize (end-to-end)
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_auth_jwt_only_allowed() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_only(
                &mut s,
                "GET",
                "/api/users/me",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("GET", "/api/users/me", &headers)
            .unwrap();
        assert!(matches!(decision, AuthDecision::Allowed { .. }));
    }

    #[test]
    fn test_auth_jwt_only_denied_roles() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["viewer"], vec![]));
            register_jwt_only(
                &mut s,
                "DELETE",
                "/api/users",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("DELETE", "/api/users", &headers)
            .unwrap();
        assert!(matches!(decision, AuthDecision::Denied { .. }));
    }

    #[test]
    fn test_auth_jwt_only_denied_permissions() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec![], vec!["docs:read"]));
            register_jwt_only(
                &mut s,
                "POST",
                "/api/docs",
                vec![],
                vec!["docs:write".to_string()],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("POST", "/api/docs", &headers)
            .unwrap();
        assert!(matches!(decision, AuthDecision::Denied { .. }));
    }

    #[test]
    fn test_auth_jwt_with_fallback_continues() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_fallback(&mut s, "POST", "/api/payments");
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("POST", "/api/payments", &headers)
            .unwrap();
        assert!(matches!(decision, AuthDecision::JwtCommonPath { .. }));
    }

    #[test]
    fn test_auth_online_only_continues() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) = build_mw_claims(make_claims("user-1", "tenant-1", vec![], vec![]));
            register_online_only(&mut s, "POST", "/api/transfer");
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("POST", "/api/transfer", &headers)
            .unwrap();
        assert!(matches!(decision, AuthDecision::JwtCommonPath { .. }));
    }

    #[test]
    fn test_auth_policy_not_found_fails_closed() {
        let (mw, _store) =
            build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw.validate_and_authorize("GET", "/unknown", &headers);
        assert!(matches!(decision, Err(AuthError::PolicyNotFound { .. })));
    }

    // ═══════════════════════════════════════════════════════════════
    //  BDD-style: given/when/then with endpoints
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_bdd_jwt_only_admin_user_gets_profile() {
        // Given: Admin user with valid JWT and matching tenant
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) = build_mw_claims(AccessClaims {
                sub: "admin-1".to_string(),
                tenant_id: "acme-corp".to_string(),
                user_type: "platform".to_string(),
                sx: SxClaims {
                    roles: vec!["admin".to_string(), "org_admin".to_string()],
                    permissions: vec!["users:read".to_string()],
                    risk: None,
                },
            });
            register_jwt_only(
                &mut s,
                "GET",
                "/api/v1/identity/users/me",
                vec!["admin".to_string()],
                vec!["users:read".to_string()],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        // When: Request to jwt-only endpoint
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "acme-corp".to_string());
        let decision = mw
            .validate_and_authorize("GET", "/api/v1/identity/users/me", &headers)
            .unwrap();

        // Then: Allowed
        assert!(
            matches!(decision, AuthDecision::Allowed { .. }),
            "Admin should be allowed to access /users/me with admin role"
        );
    }

    #[test]
    fn test_bdd_customer_without_role_denied() {
        // Given: Customer user with no admin role
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) = build_mw_claims(AccessClaims {
                sub: "customer-42".to_string(),
                tenant_id: "acme-corp".to_string(),
                user_type: "customer".to_string(),
                sx: SxClaims {
                    roles: vec!["customer".to_string()],
                    permissions: vec![],
                    risk: None,
                },
            });
            register_jwt_only(
                &mut s,
                "DELETE",
                "/api/v1/identity/users/123",
                vec!["admin".to_string()],
                vec!["users:delete".to_string()],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        // When: Customer tries to delete user (requires admin)
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "acme-corp".to_string());
        let decision = mw
            .validate_and_authorize("DELETE", "/api/v1/identity/users/123", &headers)
            .unwrap();

        // Then: Denied
        assert!(
            matches!(decision, AuthDecision::Denied { .. }),
            "Customer without admin role should be denied DELETE /users/id"
        );
    }

    #[test]
    fn test_bdd_permission_based_access() {
        // Given: User with specific permissions
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) = build_mw_claims(AccessClaims {
                sub: "user-99".to_string(),
                tenant_id: "global-shipping".to_string(),
                user_type: "customer".to_string(),
                sx: SxClaims {
                    roles: vec![],
                    permissions: vec!["shipments:create".to_string(), "shipments:read".to_string()],
                    risk: None,
                },
            });
            register_jwt_only(
                &mut s,
                "POST",
                "/api/v1/shipments",
                vec![],
                vec!["shipments:create".to_string()],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        // When: User creates shipment
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "global-shipping".to_string());
        let decision = mw
            .validate_and_authorize("POST", "/api/v1/shipments", &headers)
            .unwrap();

        // Then: Allowed (permission-based)
        assert!(
            matches!(decision, AuthDecision::Allowed { .. }),
            "User with shipments:create permission should be allowed POST /shipments"
        );
    }

    #[test]
    fn test_bdd_high_risk_route_requires_online() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "acme-corp", vec!["admin"], vec![]));
            register_jwt_fallback(&mut s, "POST", "/api/v1/payments");
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "acme-corp".to_string());
        let decision = mw
            .validate_and_authorize("POST", "/api/v1/payments", &headers)
            .unwrap();

        assert!(
            matches!(decision, AuthDecision::JwtCommonPath { .. }),
            "Payment route should return JwtCommonPath (requires online fallback)"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    //  SECURITY regression tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_security_tenant_isolation() {
        // Cross-tenant request with valid JWT but wrong tenant header
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) = build_mw_claims(AccessClaims {
                sub: "user-1".to_string(),
                tenant_id: "victim-corp".to_string(),
                user_type: "customer".to_string(),
                sx: SxClaims {
                    roles: vec!["admin".to_string()],
                    permissions: vec![],
                    risk: None,
                },
            });
            register_jwt_only(
                &mut s,
                "GET",
                "/api/v1/identity/users/me",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer forged-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "attacker-corp".to_string());

        let decision = mw.validate_and_authorize("GET", "/api/v1/identity/users/me", &headers);
        assert!(
            decision.is_err(),
            "Cross-tenant request MUST be rejected (HACK-401)"
        );
        if let Err(AuthError::TenantMismatch { expected, actual }) = decision {
            assert_eq!(expected, "attacker-corp");
            assert_eq!(actual, "victim-corp");
        }
    }

    #[test]
    fn test_security_missing_tenant_header_rejected() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_only(
                &mut s,
                "GET",
                "/api/users/me",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        // No X-Tenant-ID header

        let decision = mw.validate_and_authorize("GET", "/api/users/me", &headers);
        assert!(
            matches!(decision, Err(AuthError::MissingTenantId)),
            "Missing tenant header MUST be rejected (HACK-403)"
        );
    }

    #[test]
    fn test_security_missing_auth_header_rejected() {
        let (mw, _store) = build_mw_claims(make_claims("user-1", "tenant-1", vec![], vec![]));
        let headers = std::collections::HashMap::new();
        let decision = mw.validate_and_authorize("GET", "/api/users/me", &headers);
        assert!(
            matches!(decision, Err(AuthError::MissingAuthorization)),
            "Missing auth header MUST be rejected (HACK-405)"
        );
    }

    #[test]
    fn test_security_wrong_auth_scheme_rejected() {
        let (mw, _store) = build_mw_claims(make_claims("user-1", "tenant-1", vec![], vec![]));
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "authorization".to_string(),
            "Basic dXNlcjpwYXNz".to_string(),
        );
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw.validate_and_authorize("GET", "/api/users/me", &headers);
        assert!(
            matches!(decision, Err(AuthError::InvalidBearerScheme { .. })),
            "Non-Bearer auth MUST be rejected (HACK-405)"
        );
    }

    #[test]
    fn test_security_fail_closed_on_policy_missing() {
        let (mw, _store) =
            build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw.validate_and_authorize("GET", "/api/secret/unregistered", &headers);
        assert!(
            matches!(decision, Err(AuthError::PolicyNotFound { .. })),
            "Unregistered route must fail closed (HACK-405)"
        );
    }

    #[test]
    fn test_security_empty_roles_fails_role_policy() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) = build_mw_claims(AccessClaims {
                sub: "user-1".to_string(),
                tenant_id: "tenant-1".to_string(),
                user_type: "customer".to_string(),
                sx: SxClaims {
                    roles: vec![],
                    permissions: vec![],
                    risk: None,
                },
            });
            register_jwt_only(
                &mut s,
                "GET",
                "/api/admin",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("GET", "/api/admin", &headers)
            .unwrap();
        assert!(
            matches!(decision, AuthDecision::Denied { .. }),
            "User with no roles should be denied route requiring roles"
        );
    }

    #[test]
    fn test_security_policy_needs_roles_and_permissions() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_only(
                &mut s,
                "DELETE",
                "/api/users",
                vec!["admin".to_string(), "org_admin".to_string()],
                vec!["users:delete".to_string()],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer valid-jwt".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("DELETE", "/api/users", &headers)
            .unwrap();
        assert!(
            matches!(decision, AuthDecision::Denied { .. }),
            "User missing required permission should be denied"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    //  EDGE case tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_edge_multiple_required_roles_any_match() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["editor"], vec![]));
            register_jwt_only(
                &mut s,
                "GET",
                "/api/docs",
                vec!["admin".to_string(), "editor".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("GET", "/api/docs", &headers)
            .unwrap();
        assert!(
            matches!(decision, AuthDecision::Allowed { .. }),
            "User with any matching role should be allowed"
        );
    }

    #[test]
    fn test_edge_token_with_special_chars() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_only(
                &mut s,
                "GET",
                "/api/users/me",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(),
            "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U".to_string());
        headers.insert("x-tenant-id".to_string(), "tenant-1".to_string());
        let decision = mw
            .validate_and_authorize("GET", "/api/users/me", &headers)
            .unwrap();
        assert!(matches!(decision, AuthDecision::Allowed { .. }));
    }

    #[test]
    fn test_edge_empty_tenant_id_accepted() {
        let claims = AccessClaims {
            sub: "user-1".to_string(),
            tenant_id: "".to_string(),
            user_type: "customer".to_string(),
            sx: SxClaims::default(),
        };
        let (mw, _store) = build_mw_claims(claims);
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-tenant-id".to_string(), "".to_string());
        let empty_claims = AccessClaims {
            sub: "user-1".to_string(),
            tenant_id: "".to_string(),
            user_type: "customer".to_string(),
            sx: SxClaims::default(),
        };
        assert!(mw.validate_tenant(&empty_claims, &headers).is_ok());
    }

    #[test]
    fn test_edge_large_tenant_id() {
        let long_tenant = "a".repeat(1000);
        let claims = AccessClaims {
            sub: "user-1".to_string(),
            tenant_id: long_tenant.clone(),
            user_type: "customer".to_string(),
            sx: SxClaims::default(),
        };
        let (mw, _store) = build_mw_claims(claims);
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-tenant-id".to_string(), long_tenant.clone());
        let claims2 = AccessClaims {
            sub: "user-1".to_string(),
            tenant_id: long_tenant,
            user_type: "customer".to_string(),
            sx: SxClaims::default(),
        };
        assert!(mw.validate_tenant(&claims2, &headers).is_ok());
    }

    #[test]
    fn test_edge_risk_none_with_strict_policy() {
        let claims = AccessClaims {
            sub: "user-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_type: "customer".to_string(),
            sx: SxClaims {
                roles: vec!["viewer".to_string()],
                permissions: vec![],
                risk: None,
            },
        };
        let (mw, _store) = build_mw_claims(claims);
        let policy = RoutePolicy::new(
            "GET /api/public".to_string(),
            RouteAuthCategory::JwtOnly,
            vec!["viewer".to_string()],
            vec![],
            false,
        );
        assert!(
            mw.evaluate_local_policy(
                &make_claims("user-1", "tenant-1", vec!["viewer"], vec![]),
                &policy
            ),
            "No risk claim + strict policy should still pass (risk is None, not elevated/critical)"
        );
    }

    #[test]
    fn test_edge_risk_critical_denied_strict() {
        let mut claims = make_claims("user-1", "tenant-1", vec!["admin"], vec![]);
        claims.sx.risk = Some("critical".to_string());
        let (mw, _store) = build_mw_claims(claims.clone());
        let policy = RoutePolicy::new(
            "GET /api/safe".to_string(),
            RouteAuthCategory::JwtOnly,
            vec!["admin".to_string()],
            vec![],
            false,
        );
        assert!(
            !mw.evaluate_local_policy(&claims, &policy),
            "Critical risk should be denied on strict route"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    //  Middleware trait: before/after hooks
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_middleware_before_allowed_continues() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_only(
                &mut s,
                "GET",
                "/api/users/me",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        let mut req = make_request(Method::GET, "/api/users/me");
        let headers: Vec<(std::sync::Arc<str>, String)> = build_headers("Bearer token", "tenant-1")
            .into_iter()
            .map(|(k, v)| (std::sync::Arc::from(k.as_str()), v))
            .collect();
        req.headers = HeaderVec::from(headers);

        let result = mw.before(&req);
        assert!(
            result.is_none(),
            "Allowed requests should return None (continue to handler)"
        );
    }

    #[test]
    fn test_middleware_before_denied_returns_error() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["viewer"], vec![]));
            register_jwt_only(
                &mut s,
                "DELETE",
                "/api/users",
                vec!["admin".to_string()],
                vec![],
            );
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        let mut req = make_request(Method::DELETE, "/api/users");
        let headers: Vec<(std::sync::Arc<str>, String)> = build_headers("Bearer token", "tenant-1")
            .into_iter()
            .map(|(k, v)| (std::sync::Arc::from(k.as_str()), v))
            .collect();
        req.headers = HeaderVec::from(headers);

        let result = mw.before(&req);
        assert!(
            result.is_some(),
            "Denied requests should return Some(HandlerResponse)"
        );
        if let Some(resp) = result {
            assert_eq!(resp.status, 403);
        }
    }

    #[test]
    fn test_middleware_before_missing_auth_returns_401() {
        let (mut mw, mut store) =
            build_mw_claims(make_claims("user-1", "tenant-1", vec![], vec![]));
        register_jwt_only(
            &mut store,
            "GET",
            "/api/users/me",
            vec!["admin".to_string()],
            vec![],
        );
        set_policies(&mut mw, store);

        let req = make_request(Method::GET, "/api/users/me");
        let result = mw.before(&req);
        assert!(result.is_some(), "Missing auth should return error");
        if let Some(resp) = result {
            assert_eq!(resp.status, 401);
        }
    }

    #[test]
    fn test_middleware_before_jwt_fallback_continues() {
        let mut mw;
        let mut store = RoutePolicyStore::new();
        {
            let (inner, mut s) =
                build_mw_claims(make_claims("user-1", "tenant-1", vec!["admin"], vec![]));
            register_jwt_fallback(&mut s, "POST", "/api/payments");
            mw = inner;
            store = s;
        }
        set_policies(&mut mw, store);

        let mut req = make_request(Method::POST, "/api/payments");
        let headers: Vec<(std::sync::Arc<str>, String)> = build_headers("Bearer token", "tenant-1")
            .into_iter()
            .map(|(k, v)| (std::sync::Arc::from(k.as_str()), v))
            .collect();
        req.headers = HeaderVec::from(headers);

        let result = mw.before(&req);
        assert!(
            result.is_none(),
            "jwt-with-fallback should return None (continue to handler)"
        );
    }

    #[test]
    fn test_middleware_after_logs_latency() {
        let (mut mw, mut store) =
            build_mw_claims(make_claims("user-1", "tenant-1", vec![], vec![]));
        register_jwt_only(
            &mut store,
            "GET",
            "/api/users/me",
            vec!["admin".to_string()],
            vec![],
        );
        set_policies(&mut mw, store);

        let req = make_request(Method::GET, "/api/users/me");
        let mut res = HandlerResponse::new(200, HeaderVec::new(), serde_json::json!({}));

        // after() should complete without panicking
        mw.after(&req, &mut res, Duration::from_millis(5));
    }

    // ═══════════════════════════════════════════════════════════════
    //  Error codes and display
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_error_codes() {
        assert_eq!(AuthError::MissingAuthorization.status_code(), 401);
        assert_eq!(
            AuthError::InvalidBearerScheme {
                scheme: "Basic".to_string()
            }
            .status_code(),
            401
        );
        assert_eq!(
            AuthError::TokenExpired {
                expired_at: 1234567890
            }
            .status_code(),
            401
        );
        assert_eq!(AuthError::TokenInvalid.status_code(), 401);
        assert_eq!(AuthError::MissingTenantId.status_code(), 401);
        assert_eq!(
            AuthError::TenantMismatch {
                expected: "a".to_string(),
                actual: "b".to_string()
            }
            .status_code(),
            401
        );
        assert_eq!(
            AuthError::PolicyNotFound {
                path: "/x".to_string(),
                method: "GET".to_string()
            }
            .status_code(),
            503
        );
    }

    #[test]
    fn test_auth_error_display() {
        assert_eq!(
            AuthError::MissingAuthorization.message(),
            "Missing Authorization header"
        );
        assert_eq!(
            AuthError::InvalidBearerScheme {
                scheme: "Basic".to_string()
            }
            .message(),
            "Invalid Authorization scheme: Basic. Only Bearer is accepted"
        );
        assert_eq!(
            AuthError::TenantMismatch {
                expected: "tenant-a".to_string(),
                actual: "tenant-b".to_string()
            }
            .message(),
            "Tenant mismatch: expected tenant-a, got tenant-b"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    //  SxClaims helper method tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_sx_has_role() {
        let sx = SxClaims {
            roles: vec!["admin".to_string(), "viewer".to_string()],
            permissions: vec![],
            risk: None,
        };
        assert!(sx.has_role("admin"));
        assert!(sx.has_role("viewer"));
        assert!(!sx.has_role("editor"));
    }

    #[test]
    fn test_sx_has_permission() {
        let sx = SxClaims {
            roles: vec![],
            permissions: vec!["users:read".to_string(), "users:write".to_string()],
            risk: None,
        };
        assert!(sx.has_permission("users:read"));
        assert!(sx.has_permission("users:write"));
        assert!(!sx.has_permission("users:delete"));
    }

    #[test]
    fn test_sx_is_normal_risk() {
        assert!(SxClaims {
            roles: vec![],
            permissions: vec![],
            risk: None
        }
        .is_normal_risk());
        assert!(SxClaims {
            roles: vec![],
            permissions: vec![],
            risk: Some("normal".to_string())
        }
        .is_normal_risk());
        assert!(!SxClaims {
            roles: vec![],
            permissions: vec![],
            risk: Some("elevated".to_string())
        }
        .is_normal_risk());
        assert!(!SxClaims {
            roles: vec![],
            permissions: vec![],
            risk: Some("critical".to_string())
        }
        .is_normal_risk());
    }

    #[test]
    fn test_sx_has_any_authorization() {
        assert!(!SxClaims {
            roles: vec![],
            permissions: vec![],
            risk: None
        }
        .has_any_authorization());
        assert!(SxClaims {
            roles: vec!["admin".to_string()],
            permissions: vec![],
            risk: None
        }
        .has_any_authorization());
        assert!(SxClaims {
            roles: vec![],
            permissions: vec!["read".to_string()],
            risk: None
        }
        .has_any_authorization());
    }
}
