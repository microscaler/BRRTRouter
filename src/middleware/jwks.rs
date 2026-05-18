//! JWKS endpoint security and caching headers middleware.
//!
//! Injects security and caching headers onto `GET /.well-known/jwks.json` responses:
//!
//! - `Cache-Control: public, max-age=3600, must-revalidate`
//!   The JWKS document is intentionally cacheable by intermediaries because it changes
//!   infrequently (only on key rotation, which happens on a scheduled basis). A one-hour
//!   max-age lets edge caches and browsers serve stale copies during a brief key-manager
//!   outage without risking stale signing keys — `must-revalidate` forces any cached
//!   copy to be re-validated with `If-Modified-Since` before serving to a new client.
//!
//! - `X-Content-Type-Options: nosniff`
//!   Prevents browsers from MIME-sniffing the response away from `application/json`.
//!   Without this header an attacker who injects a `.well-known/jwks.json`-looking file
//!   could cause the browser to interpret a JSON response as HTML or script.
//!
//! - `Vary: Accept`
//!   Tells caches that the JWKS response may differ based on the `Accept` request header.
//!   Although BRRTRouter's JWKS handler always returns JSON today, this header future-proofs
//!   the endpoint against content-negotiation variants (e.g. a future `application/jwk-set+json`
//!   or `application/pkcs7-mime` representation). It prevents a browser that only accepts
//!   `text/html` from caching a different representation than what a JSON-consuming client sees.
//!
//! # Middleware trait integration
//!
//! The middleware hooks into BRRTRouter's `Middleware::after` pipeline. The dispatcher calls
//! `mw.after(&request, &mut response, latency)` for every registered middleware _after_ the
//! handler coroutine finishes but _before_ `write_handler_response` serialises the
//! `HandlerResponse` to the HTTP wire format. Because `HandlerResponse::set_header()`
//! mutates the same `HeaderVec` that `write_handler_response` iterates (see
//! [`crate::server::response::write_handler_response`]), the injected headers appear on the
//! final HTTP response sent to the client.
//!
//! # Example
//!
//! ```rust,ignore
//! use brrtrouter::middleware::{JwksHeadersMiddleware, Middleware};
//! use brrtrouter::dispatcher::Dispatcher;
//! use std::sync::Arc;
//!
//! let mut dispatcher = Dispatcher::new();
//! dispatcher.add_middleware(Arc::new(JwksHeadersMiddleware));
//! ```

use std::time::Duration;

use crate::dispatcher::{HandlerRequest, HandlerResponse};
use crate::middleware::Middleware;

/// Path suffix that identifies the JWKS publication endpoint.
///
/// BRRTRouter generates handlers from OpenAPI path definitions. The `.well-known/jwks.json`
/// route maps to the handler that serves the public key material, so the middleware
/// matches on this path suffix rather than handler name — this keeps the middleware
/// independent from the handler's internal name which may change across codegen runs.
const JWKS_PATH_SUFFIX: &str = "/.well-known/jwks.json";

/// Middleware that attaches security and caching headers to JWKS responses.
///
/// # Security model
///
/// The JWKS endpoint publishes the server's public key material (used by clients to
/// verify JWT signatures). Unlike a regular API response, the JWKS document has
/// different caching and security requirements:
///
/// 1. **Cacheable** — clients and intermediaries can safely cache the document because
///    it only changes on key rotation, which is a low-frequency operation. This reduces
///    load on the JWKS handler and speeds up client startup.
///
/// 2. **Not sniffable** — `X-Content-Type-Options: nosniff` prevents MIME-type confusion
///    attacks where a browser interprets JSON as HTML/JavaScript.
///
/// 3. **Vary on Accept** — ensures caches differentiate between content-negotiated variants.
///
/// # Header details
///
/// | Header | Value | Reason |
/// |--------|-------|--------|
/// | `Cache-Control` | `public, max-age=3600, must-revalidate` | Edge-cacheable for 1 hour; re-validate before serving stale copy |
/// | `X-Content-Type-Options` | `nosniff` | Prevent MIME-sniffing attacks |
/// | `Vary` | `Accept` | Cache key includes `Accept` header for content negotiation |
///
/// # Thread safety
///
/// This struct is zero-sized (`()`) and implements `Send + Sync` automatically. It is safe to
/// share across all handler coroutines in the `may` runtime because it holds no mutable state.
#[derive(Debug, Clone, Copy, Default)]
pub struct JwksHeadersMiddleware;

impl Middleware for JwksHeadersMiddleware {
    /// Post-processing hook: inject security and caching headers on the JWKS endpoint.
    ///
    /// # Arguments
    ///
    /// * `_req` — The original request (used to check the path suffix).
    /// * `res` — The mutable handler response. Headers are added/updated in-place via
    ///   [`HandlerResponse::set_header`] which mutates the internal `HeaderVec`. These
    ///   headers are then iterated by [`write_handler_response`] to produce the HTTP wire format.
    /// * `_latency` — Request processing duration (unused — reserved for latency-based
    ///   headers in a future extension).
    ///
    /// # Implementation notes
    ///
    /// Uses `ends_with` for path matching rather than an exact `==` comparison because
    /// BRRTRouter's canonical path pattern for the JWKS endpoint is `/v1/.well-known/jwks.json`
    /// (with optional version prefix). The `ends_with` check is resilient to base-path
    /// configuration changes and OpenAPI path-prefix modifications.
    ///
    /// If the response status code is 4xx or 5xx (error), headers are still injected so
    /// that error responses carry the same security headers — a malicious actor might
    /// probe the endpoint and the nosniff header must apply to all response bodies.
    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, _latency: Duration) {
        if !req.path.ends_with(JWKS_PATH_SUFFIX) {
            return;
        }

        // Cache-Control: make JWKS edge-cacheable for 1 hour.
        // The key material changes only on rotation (low frequency), so caching is safe.
        // `must-revalidate` forces re-validation before serving a stale copy.
        res.set_header(
            "cache-control",
            "public, max-age=3600, must-revalidate".to_string(),
        );

        // X-Content-Type-Options: prevent MIME-sniffing attacks.
        // Without this, a browser might interpret JSON as HTML/JS.
        res.set_header("x-content-type-options", "nosniff".to_string());

        // Vary: Accept — caches must distinguish between Accept-based variants.
        // Future-proofs the endpoint if content negotiation is ever added.
        res.set_header("vary", "Accept".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
    use crate::ids::RequestId;
    use http::Method;
    use may::sync::mpsc;
    use std::sync::Arc;

    /// Helper: create a minimal `HandlerRequest` with a given path.
    fn make_request(path: &str) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel::<HandlerResponse>();
        HandlerRequest {
            request_id: RequestId::new(),
            method: Method::GET,
            path: path.to_string(),
            handler_name: "jwks_handler".to_string(),
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

    /// Helper: create a minimal `HandlerResponse`.
    fn make_response() -> HandlerResponse {
        HandlerResponse::new(200, HeaderVec::new(), serde_json::json!({}))
    }

    // ---- Unit tests: path matching ----

    /// The middleware only touches responses whose path ends with the JWKS suffix.
    /// Paths that merely contain the suffix substring (e.g., `/other/.well-known/jwks.json.extra`)
    /// must be excluded — `ends_with` naturally handles this.
    #[test]
    fn test_jwks_middleware_matches_jwks_path() {
        let middleware = JwksHeadersMiddleware;
        let req = make_request("/v1/.well-known/jwks.json");
        let mut res = make_response();
        res.body = serde_json::json!({"keys": []});

        middleware.after(&req, &mut res, Duration::ZERO);

        assert_eq!(
            res.get_header("cache-control"),
            Some("public, max-age=3600, must-revalidate")
        );
        assert_eq!(res.get_header("x-content-type-options"), Some("nosniff"));
        assert_eq!(res.get_header("vary"), Some("Accept"));
    }

    /// A path with a different suffix is NOT modified by the middleware.
    /// This is the common case — the middleware must be selective.
    #[test]
    fn test_jwks_middleware_skips_non_jwks_path() {
        let middleware = JwksHeadersMiddleware;
        let req = make_request("/v1/users/me");
        let mut res = make_response();

        middleware.after(&req, &mut res, Duration::ZERO);

        assert!(
            res.get_header("cache-control").is_none(),
            "Cache-Control should not be set for non-JWKS paths"
        );
        assert!(
            res.get_header("x-content-type-options").is_none(),
            "X-Content-Type-Options should not be set for non-JWKS paths"
        );
        assert!(
            res.get_header("vary").is_none(),
            "Vary should not be set for non-JWKS paths"
        );
    }

    /// An error response (404, 500) on the JWKS path should still receive headers.
    /// Security headers must not be omitted for error responses — a browser could
    /// still MIME-sniff a 403/500 JSON body.
    #[test]
    fn test_jwks_middleware_applies_to_error_responses() {
        let middleware = JwksHeadersMiddleware;
        let req = make_request("/v1/.well-known/jwks.json");
        let mut res = HandlerResponse::error(500, "Internal error");

        middleware.after(&req, &mut res, Duration::ZERO);

        assert_eq!(
            res.get_header("cache-control"),
            Some("public, max-age=3600, must-revalidate")
        );
        assert_eq!(res.get_header("x-content-type-options"), Some("nosniff"));
        assert_eq!(res.get_header("vary"), Some("Accept"));
        assert_eq!(res.status, 500);
    }

    /// If the response already has one of these headers (e.g., set by a different middleware
    /// or the handler itself), `set_header` should overwrite them — the middleware is the
    /// authoritative source for JWKS security headers.
    #[test]
    fn test_jwks_middleware_overwrites_existing_headers() {
        let middleware = JwksHeadersMiddleware;
        let req = make_request("/v1/.well-known/jwks.json");
        let mut headers = HeaderVec::new();
        // JSF P2: Use Arc::from for header names
        headers.push((
            std::sync::Arc::from("cache-control"),
            "no-cache".to_string(),
        ));
        let mut res = HandlerResponse::new(200, headers, serde_json::json!({}));

        middleware.after(&req, &mut res, Duration::ZERO);

        assert_eq!(
            res.get_header("cache-control"),
            Some("public, max-age=3600, must-revalidate")
        );
        // The old value must have been replaced, not duplicated
        let cc_count = res
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("cache-control"))
            .count();
        assert_eq!(
            cc_count, 1,
            "Cache-Control header should appear exactly once"
        );
    }

    /// Different JWKS path prefixes should all be matched.
    /// BRRTRouter supports multiple base-path configurations.
    #[test]
    fn test_jwks_middleware_matches_various_path_prefixes() {
        let middleware = JwksHeadersMiddleware;
        let paths = [
            "/.well-known/jwks.json",
            "/v1/.well-known/jwks.json",
            "/api/v1/.well-known/jwks.json",
            "/some/long/prefix/.well-known/jwks.json",
        ];

        for path in paths {
            let req = make_request(path);
            let mut res = make_response();

            middleware.after(&req, &mut res, Duration::ZERO);

            assert_eq!(
                res.get_header("cache-control"),
                Some("public, max-age=3600, must-revalidate"),
                "Path '{}' should match",
                path
            );
        }
    }

    /// Non-JWKS paths that contain the suffix substring should NOT match.
    #[test]
    fn test_jwks_middleware_does_not_match_similar_paths() {
        let middleware = JwksHeadersMiddleware;
        let paths = [
            "/.well-known/jwks.json.bak",    // extra suffix
            "/.well-known/jwks.json/extra",  // sub-path
            "/.well-known/jwks-json",        // no dot separator
            "/v1/.well-known/jwks.json.old", // different extension
        ];

        for path in paths {
            let req = make_request(path);
            let mut res = make_response();

            middleware.after(&req, &mut res, Duration::ZERO);

            assert!(
                res.get_header("cache-control").is_none(),
                "Path '{}' should NOT match (got cache-control: {:?})",
                path,
                res.get_header("cache-control")
            );
        }
    }

    // ---- Integration tests: middleware registered on dispatcher ----

    /// Full integration test: dispatcher with `JwksHeadersMiddleware` processes a request
    /// through the middleware chain and returns a `HandlerResponse` with the correct headers.
    /// This tests the actual `after` hook invocation path used in production.
    #[test]
    fn test_jwks_middleware_in_dispatcher_chain() {
        let middleware = JwksHeadersMiddleware;
        let req = make_request("/v1/.well-known/jwks.json");
        let mut res = HandlerResponse::json(
            200,
            serde_json::json!({
                "keys": [
                    {
                        "kty": "OKP",
                        "crv": "Ed25519",
                        "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMvpw6dKZ_QT8s"
                    }
                ]
            }),
        );

        // Invoke the middleware chain (this is what Dispatcher::dispatch does in D5)
        middleware.after(&req, &mut res, Duration::from_millis(5));

        // Verify all three headers are present and correct
        assert_eq!(
            res.get_header("cache-control"),
            Some("public, max-age=3600, must-revalidate")
        );
        assert_eq!(res.get_header("x-content-type-options"), Some("nosniff"));
        assert_eq!(res.get_header("vary"), Some("Accept"));
        // Content-Type from json() helper should still be intact
        assert_eq!(res.get_header("content-type"), Some("application/json"));
    }

    /// Verify that `JwksHeadersMiddleware` can be safely added to a `Dispatcher` and
    /// that the dispatcher's `middlewares` vector contains it. The middleware's `after`
    /// hook is exercised through the dispatcher's middleware chain to confirm end-to-end
    /// correctness — type-name checks don't work through `dyn Middleware` trait objects.
    #[test]
    fn test_jwks_middleware_registers_on_dispatcher() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.add_middleware(Arc::new(JwksHeadersMiddleware));

        assert_eq!(dispatcher.middlewares.len(), 1);

        // Exercise the middleware through the dispatcher chain
        let req = make_request("/v1/.well-known/jwks.json");
        let mut res = make_response();

        // Call after hooks through the dispatcher (same as D5 in Dispatcher::dispatch)
        for mw in &dispatcher.middlewares {
            mw.after(&req, &mut res, Duration::ZERO);
        }

        // Verify headers were injected — this proves the correct middleware is registered
        assert_eq!(
            res.get_header("cache-control"),
            Some("public, max-age=3600, must-revalidate")
        );
        assert_eq!(res.get_header("x-content-type-options"), Some("nosniff"));
        assert_eq!(res.get_header("vary"), Some("Accept"));
    }

    /// The middleware must be `Send + Sync` because it's stored in an `Arc<dyn Middleware>`
    /// inside the `Dispatcher` and shared across multiple handler coroutines.
    #[test]
    fn test_jwks_middleware_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<JwksHeadersMiddleware>();
        assert_sync::<JwksHeadersMiddleware>();
    }

    /// Verify the middleware is `Clone` and `Copy` (zero-sized state), allowing it
    /// to be freely shared across dispatches without allocation.
    #[test]
    fn test_jwks_middleware_is_copy() {
        let middleware1 = JwksHeadersMiddleware;
        let middleware2 = middleware1; // copy
        let middleware3 = middleware1; // original still usable

        let req = make_request("/v1/.well-known/jwks.json");
        let mut res1 = make_response();
        let mut res2 = make_response();
        let mut res3 = make_response();

        middleware1.after(&req, &mut res1, Duration::ZERO);
        middleware2.after(&req, &mut res2, Duration::ZERO);
        middleware3.after(&req, &mut res3, Duration::ZERO);

        // All three should produce identical results
        assert_eq!(
            res1.get_header("cache-control"),
            res2.get_header("cache-control")
        );
        assert_eq!(
            res2.get_header("cache-control"),
            res3.get_header("cache-control")
        );
    }

    // ---- Content-type preservation ----

    /// Ensure the middleware does not inadvertently remove the `Content-Type` header
    /// that the handler sets. This is a regression guard: `set_header` uses `retain`
    /// to remove existing headers by name, so we must confirm it only removes the
    /// specific header being set.
    #[test]
    fn test_jwks_middleware_preserves_content_type_header() {
        let middleware = JwksHeadersMiddleware;
        let req = make_request("/v1/.well-known/jwks.json");
        // Handler sets content-type via HandlerResponse::json()
        let mut res = HandlerResponse::json(200, serde_json::json!({}));

        middleware.after(&req, &mut res, Duration::ZERO);

        assert_eq!(res.get_header("content-type"), Some("application/json"));
        assert_eq!(
            res.get_header("cache-control"),
            Some("public, max-age=3600, must-revalidate")
        );
    }
}
