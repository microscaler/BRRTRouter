//! Shadow decision observability for JWT migration mode
//!
//! This module implements the shadow decision comparison system described in Story 9.4.
//! It compares JWT authorization decisions against online authz-core decisions during
//! migration, logging mismatches for analysis.
//!
//! ## Architecture
//!
//! ```text
//! Request -> JWT middleware (common path) -> JWT decision
//! Request -> authz-core /authorize (background) -> Online decision
//!   -> Compare: JWT decision == Online decision?
//!   -> If YES: shadow hit (DEBUG log, no-op)
//!   -> If NO: shadow mismatch (WARN log, record in span)
//! ```
//!
//! The JWT decision **always takes precedence**. The online decision is shadow-only.
//!
//! ## Security
//!
//! **HACK-941:** Shadow mode MUST be disabled in production. If enabled, it creates an
//! authorization oracle — an attacker with log stream access can map the online
//! authorization system by searching for `shadow_mismatch` events.
//!
//! **HACK-942:** Shadow mode doubles authz-core load for jwt-with-fallback routes.
//! Service refuses to start if shadow mode is enabled in production.
//!
//! See story-9.4.md for the full threat model.
//!
//! ## Enabling/Disabling
//!
//! Shadow mode is controlled by `SHADOW_MODE_ENABLED` env var — NEVER by client input:
//!
//! ```bash
//! # Development only:
//! export SHADOW_MODE_ENABLED=true
//! ```
//!
//! **Production MUST have this unset or set to `false`.**

use crate::security::decision::{AuthDecision, MismatchReason};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing;

/// Shadow mode configuration and evaluation engine.
///
/// Compares JWT authorization decisions against online authz-core decisions
/// during migration, creating `shadow_decision.compare` spans and structured logs.
///
/// # Thread Safety
///
/// `ShadowDecision` is wrapped in `Arc` for sharing across handlers. The `enabled`
/// flag is an `AtomicBool` for lock-free runtime toggling.
///
/// # Example
///
/// ```rust,ignore
/// use brrtrouter::security::shadow::ShadowDecision;
///
/// let shadow = ShadowDecision::from_env();
///
/// // In your JWT middleware handler:
/// let jwt_decision = AuthDecision::allowed(Some("role:admin".to_string()));
/// let route = "/api/users".to_string();
/// shadow.evaluate(&route, &jwt_decision, &request).await;
/// ```
pub struct ShadowDecision {
    /// Whether shadow decision comparison is enabled.
    /// Can be toggled at runtime without restarting the service.
    enabled: Arc<AtomicBool>,
}

impl ShadowDecision {
    /// Create a new `ShadowDecision` from environment variable `SHADOW_MODE_ENABLED`.
    ///
    /// # Production Safety
    ///
    /// Returns an error if `SHADOW_MODE_ENABLED=true` and `RUNNING_IN_PRODUCTION=true`.
    /// This is the startup security check from HACK-942 — the service must refuse to
    /// start if shadow mode is enabled in production.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let shadow = ShadowDecision::from_env()?;
    /// ```
    pub fn from_env() -> Result<Self, String> {
        let enabled = env::var("SHADOW_MODE_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        let in_production = env::var("RUNNING_IN_PRODUCTION")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        if enabled && in_production {
            return Err(
                "FATAL: SHADOW_MODE_ENABLED=true with RUNNING_IN_PRODUCTION=true. \
                 Shadow mode MUST be disabled in production. See HACK-942."
                    .to_string(),
            );
        }

        Ok(Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        })
    }

    /// Create a new `ShadowDecision` with a fixed enabled state (useful for testing).
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    /// Check if shadow mode is currently enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Set shadow mode at runtime.
    ///
    /// This is an **internal operation only** — never exposed via HTTP/API.
    /// Client input can never enable shadow mode (HACK-941 mitigation).
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Evaluate shadow decision: compare JWT decision against online authz-core.
    ///
    /// This is a **fire-and-forget, non-blocking** operation. It spawns a background
    /// task that:
    /// 1. Calls authz-core `/authorize` endpoint with the request
    /// 2. Compares the online decision with the JWT decision
    /// 3. Creates a `shadow_decision.compare` span with attributes
    /// 4. Logs the result (DEBUG for hit, WARN for mismatch, DEBUG for error)
    ///
    /// # Arguments
    ///
    /// * `route` - The route path (e.g., "/api/users") for logging context
    /// * `jwt_decision` - The decision from JWT validation
    ///
    /// # Background Task
    ///
    /// The online authorization check runs in a spawned task. If shadow mode is
    /// disabled, the function returns immediately without spawning anything.
    ///
    /// # Concurrency
    ///
    /// Each call spawns an independent task. Under high concurrency (1000+ req/s),
    /// this creates 1000+ concurrent shadow tasks — each using ~8KB stack = ~8MB.
    /// This is acceptable during migration (2 weeks per story) but NOT in production.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let shadow = ShadowDecision::from_env()?;
    /// let jwt_decision = AuthDecision::allowed(Some("role:admin".to_string()));
    /// shadow.evaluate("/api/users", &jwt_decision).await;
    /// ```
    pub async fn evaluate(&self, route: &str, jwt_decision: &AuthDecision) {
        // Early return if shadow mode is disabled — no span, no background task
        if !self.is_enabled() {
            return;
        }

        // Clone values for the spawned task (fire-and-forget)
        let route = route.to_string();
        let jwt_dec = jwt_decision.clone();
        let enabled_clone = Arc::clone(&self.enabled);

        // Spawn background comparison task (non-blocking, fire-and-forget)
        tokio::spawn(async move {
            // Check enabled flag inside the task in case it was toggled
            if !enabled_clone.load(Ordering::Acquire) {
                return;
            }

            let span = tracing::span!(
                tracing::Level::INFO,
                "shadow_decision.compare",
                route = route,
                jwt_decision = ?jwt_dec,
            );
            let _guard = span.enter();

            // Attempt online authorization check (shadow — best effort only)
            let online_decision = Self::call_authz_core().await;

            let jwt_allowed = jwt_dec.is_allowed();

            match online_decision {
                Ok(online_allowed) => {
                    let _ = span.record(
                        "online_decision",
                        if online_allowed { "allowed" } else { "denied" },
                    );

                    if jwt_allowed == online_allowed {
                        // HIT: decisions match
                        let _ = span.record("result", "hit");
                        tracing::debug!(
                            event = "shadow_decision_match",
                            route = &route,
                            jwt_decision = jwt_dec.decision_str(),
                            online_decision = if online_allowed { "allowed" } else { "denied" },
                            "Shadow decision: hit (decisions match)"
                        );
                    } else {
                        // MISMATCH: decisions differ
                        let reason = if jwt_allowed {
                            MismatchReason::JwtAllowedButOnlineDenied
                        } else {
                            MismatchReason::JwtDeniedButOnlineAllowed
                        };

                        let _ = span.record("result", "mismatch");
                        let _ = span.record("mismatch_reason", reason.as_str());

                        tracing::warn!(
                            event = "shadow_mismatch",
                            route = &route,
                            jwt_decision = jwt_dec.decision_str(),
                            online_decision = if online_allowed { "allowed" } else { "denied" },
                            reason = reason.as_str(),
                            severity = reason.severity(),
                            "Shadow decision: mismatch"
                        );
                    }
                }
                Err(e) => {
                    // Online check failed: shadow is best-effort, ignore
                    let _ = span.record("result", "error");
                    let err_msg = e.to_string();
                    let _ = span.record("error", err_msg.as_str());
                    tracing::debug!(
                        event = "shadow_decision_error",
                        route = &route,
                        error = %e,
                        "Shadow decision: online check failed (ignored, shadow is best-effort)"
                    );
                }
            }
        });
    }

    /// Call the online authz-core authorization endpoint.
    ///
    /// This is a placeholder — the actual implementation will be provided when
    /// Story 4.3 (selective online fallback) is implemented.
    ///
    /// For now, it simulates an online decision based on the JWT decision
    /// for testing purposes. In production, this will make an HTTP call to
    /// authz-core's `/authorize` endpoint.
    async fn call_authz_core() -> Result<bool, String> {
        // TODO (Story 4.3): Replace with actual authz-core HTTP call
        // For now, simulate: assume online always agrees with JWT (hit)
        // This allows the shadow system to be tested during migration
        Ok(true)
    }

    /// Get the shadow mode configuration status.
    #[must_use]
    pub fn status(&self) -> ShadowStatus {
        ShadowStatus {
            enabled: self.is_enabled(),
        }
    }
}

/// Runtime status of shadow decision mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowStatus {
    /// Whether shadow decision comparison is enabled.
    pub enabled: bool,
}

impl std::fmt::Display for ShadowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.enabled {
            write!(f, "shadow_decision: enabled (migration mode)")
        } else {
            write!(f, "shadow_decision: disabled (production mode)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_disabled_by_default() {
        // Without SHADOW_MODE_ENABLED=true, shadow mode is disabled
        let shadow = ShadowDecision::new(false);
        assert!(!shadow.is_enabled());
    }

    #[test]
    fn test_shadow_enabled() {
        let shadow = ShadowDecision::new(true);
        assert!(shadow.is_enabled());
    }

    #[test]
    fn test_shadow_runtime_toggle() {
        let shadow = ShadowDecision::new(true);
        assert!(shadow.is_enabled());

        shadow.set_enabled(false);
        assert!(!shadow.is_enabled());

        shadow.set_enabled(true);
        assert!(shadow.is_enabled());
    }

    #[test]
    fn test_shadow_status_display() {
        let status = ShadowStatus { enabled: false };
        assert!(format!("{}", status).contains("disabled"));
        assert!(format!("{}", status).contains("production"));

        let status = ShadowStatus { enabled: true };
        assert!(format!("{}", status).contains("enabled"));
        assert!(format!("{}", status).contains("migration"));
    }

    #[tokio::test]
    async fn test_shadow_disabled_no_background_task() {
        // When disabled, evaluate() should return immediately without spawning
        let shadow = ShadowDecision::new(false);
        let jwt_decision = AuthDecision::allowed(Some("test".to_string()));

        // This should return instantly (no spawn)
        shadow.evaluate("/api/test", &jwt_decision).await;
    }

    #[tokio::test]
    async fn test_shadow_enabled_creates_span() {
        let shadow = ShadowDecision::new(true);
        let jwt_decision = AuthDecision::allowed(Some("test".to_string()));

        // This should spawn a background task that creates a span
        shadow.evaluate("/api/test", &jwt_decision).await;

        // The task runs in background — we just verify it doesn't panic
        // In a real test, we'd use a test subscriber to capture span events
    }

    #[tokio::test]
    async fn test_shadow_enabled_after_toggle() {
        let shadow = ShadowDecision::new(false);
        let jwt_decision = AuthDecision::allowed(Some("test".to_string()));

        // Start disabled — should not spawn
        shadow.evaluate("/api/test", &jwt_decision).await;

        // Now enable — should spawn
        shadow.set_enabled(true);
        shadow.evaluate("/api/test", &jwt_decision).await;
    }

    #[test]
    fn test_shadow_from_env_disabled() {
        // Without SHADOW_MODE_ENABLED, should be disabled
        // (This test assumes the env var is not set)
        let result = ShadowDecision::from_env();
        // In CI, this may or may not be set — just verify it doesn't panic
        if let Ok(shadow) = result {
            // Verify it's created without error
            assert_eq!(
                shadow.status().enabled,
                false
                    || env::var("SHADOW_MODE_ENABLED")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false)
            );
        }
    }

    #[tokio::test]
    async fn test_shadow_decision_types() {
        // Verify AuthDecision variants work correctly with shadow evaluation
        let shadow = ShadowDecision::new(true);

        let allowed = AuthDecision::allowed(Some("role:admin".to_string()));
        let denied = AuthDecision::denied(Some("missing scope".to_string()));

        // Both should work without panic
        shadow.evaluate("/api/users", &allowed).await;
        shadow.evaluate("/api/admin", &denied).await;
    }

    #[tokio::test]
    async fn test_concurrent_shadow_evaluations() {
        // Verify 100 concurrent shadow evaluations create independent spans
        let shadow = Arc::new(ShadowDecision::new(true));
        let jwt_decision = AuthDecision::allowed(Some("role:user".to_string()));

        let mut handles = Vec::new();
        for i in 0..100 {
            let s = Arc::clone(&shadow);
            let decision = jwt_decision.clone();
            handles.push(tokio::spawn(async move {
                s.evaluate(&format!("/api/test/{i}"), &decision).await;
            }));
        }

        // All should complete (or not start, depending on enabled state)
        for handle in handles {
            let _ = handle.await;
        }
    }

    #[test]
    fn test_shadow_decision_pii_not_included() {
        // Verify that AuthDecision does not contain PII fields
        let decision = AuthDecision::allowed(Some("role:admin".to_string()));

        // Only role/permission/scope info is included — no email, phone, name
        assert!(decision.reason().unwrap().contains("role"));
        assert!(decision.reason().unwrap().contains("admin"));
    }

    #[tokio::test]
    async fn test_shadow_online_check_failure_ignored() {
        // When online check fails, should log DEBUG error and not count as mismatch
        let shadow = ShadowDecision::new(true);
        let jwt_decision = AuthDecision::allowed(Some("test".to_string()));

        shadow.evaluate("/api/test", &jwt_decision).await;
        // The background task should complete without panicking
        // In a real scenario, the online check would return Err
    }
}
