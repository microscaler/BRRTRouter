//! HTTP method helpers for RFC 10008 QUERY (Stories 11.1 / 11.3).
//!
//! `http::Method` has no `Method::QUERY` constant; QUERY is an extension token.
//! Routing and CORS use [`method_query`] (uppercase only). Lowercase `query` is a
//! distinct token and will not match QUERY routes.

use std::sync::OnceLock;

use http::Method;

/// RFC 10008 HTTP QUERY method (uppercase).
#[must_use]
pub fn method_query() -> Method {
    static QUERY: OnceLock<Method> = OnceLock::new();
    QUERY
        .get_or_init(|| Method::from_bytes(b"QUERY").expect("QUERY is a valid method token"))
        .clone()
}

/// `true` when `method` is exactly uppercase QUERY.
#[must_use]
pub fn is_query_method(method: &Method) -> bool {
    method == &method_query()
}

/// Methods eligible for automatic retry (safe + idempotent per RFC 9110 / RFC 10008).
///
/// Includes [`method_query`]: QUERY is defined as both safe and idempotent, so
/// retry policies may treat it like GET (Story 11.3 P6). Does **not** include
/// POST/PATCH (not idempotent) or PUT/DELETE (idempotent but not safe).
#[must_use]
pub fn method_allows_automatic_retry(method: &Method) -> bool {
    matches!(
        method.as_str(),
        "GET" | "HEAD" | "OPTIONS" | "TRACE" | "QUERY"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
    use crate::ids::RequestId;
    use crate::middleware::{CorsMiddleware, CorsMiddlewareBuilder, Middleware};
    use crate::router::{ParamVec, Router};
    use crate::spec::RouteMeta;
    use may::sync::mpsc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn route(method: Method, path: &str, handler: &str) -> RouteMeta {
        RouteMeta {
            x_service: None,
            x_brrtrouter_downstream_path: None,
            x_brrtrouter_impl: None,
            method,
            path_pattern: Arc::from(path),
            handler_name: Arc::from(handler),
            base_path: String::new(),
            parameters: Vec::new(),
            request_schema: None,
            request_body_required: false,
            request_content_types: Vec::new(),
            response_schema: None,
            example: None,
            responses: HashMap::new(),
            security: Vec::new(),
            example_name: "test_example".to_string(),
            project_slug: "test_project".to_string(),
            output_dir: PathBuf::from("test_output"),
            sse: false,
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
            cors_policy: crate::middleware::RouteCorsPolicy::Inherit,
        }
    }

    fn options_req(origin: &str, request_method: &str) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel::<HandlerResponse>();
        let mut headers = HeaderVec::new();
        headers.push((Arc::from("origin"), origin.to_string()));
        headers.push((
            Arc::from("access-control-request-method"),
            request_method.to_string(),
        ));
        HandlerRequest {
            request_id: RequestId::new(),
            method: Method::OPTIONS,
            path: "/search".to_string(),
            handler_name: "cors".to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            raw_query: None,
            headers,
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
        }
    }

    // --- Positive ---

    #[test]
    fn query_method_positive_p1_router_matches_registered_query() {
        let router = Router::new(vec![route(method_query(), "/search", "search_query")]);
        let m = router
            .route(method_query(), "/search")
            .expect("QUERY route");
        assert_eq!(m.handler_name, "search_query");
    }

    #[test]
    fn query_method_positive_p2_same_path_get_and_query() {
        let router = Router::new(vec![
            route(Method::GET, "/search", "search_get"),
            route(method_query(), "/search", "search_query"),
        ]);
        assert_eq!(
            router.route(Method::GET, "/search").unwrap().handler_name,
            "search_get"
        );
        assert_eq!(
            router
                .route(method_query(), "/search")
                .unwrap()
                .handler_name,
            "search_query"
        );
    }

    #[test]
    fn query_method_positive_p3_cors_preflight_lists_query() {
        let mw = CorsMiddleware::permissive();
        let req = options_req("https://app.example", "QUERY");
        let resp = mw.before(&req).expect("preflight response");
        assert_eq!(resp.status, 200);
        let methods = resp
            .get_header("access-control-allow-methods")
            .expect("Allow-Methods");
        assert!(
            methods.split(", ").any(|m| m == "QUERY"),
            "expected QUERY in {methods}"
        );
    }

    #[test]
    fn query_method_positive_p4_cors_allows_query_request_method() {
        let mw = CorsMiddleware::permissive();
        let req = options_req("https://app.example", "QUERY");
        let resp = mw.before(&req).expect("preflight");
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn query_method_positive_p5_uppercase_bytes() {
        let m = Method::from_bytes(b"QUERY").unwrap();
        assert!(is_query_method(&m));
        assert_eq!(m.as_str(), "QUERY");
        assert_eq!(m, method_query());
    }

    #[test]
    fn query_method_positive_p6_get_post_unaffected() {
        let router = Router::new(vec![
            route(Method::GET, "/items", "get_items"),
            route(Method::POST, "/items", "create_item"),
            route(method_query(), "/items", "query_items"),
        ]);
        assert!(router.route(Method::GET, "/items").is_some());
        assert!(router.route(Method::POST, "/items").is_some());
        assert!(router.route(method_query(), "/items").is_some());
    }

    // --- Negative ---

    #[test]
    fn query_method_negative_n1_unregistered_path() {
        let router = Router::new(vec![route(method_query(), "/search", "search_query")]);
        assert!(router.route(method_query(), "/other").is_none());
    }

    #[test]
    fn query_method_negative_n2_unknown_method_no_route() {
        let router = Router::new(vec![route(Method::GET, "/search", "get")]);
        let weird = Method::from_bytes(b"PURGE").unwrap();
        assert!(router.route(weird, "/search").is_none());
    }

    #[test]
    fn query_method_negative_n3_lowercase_query_distinct() {
        // http accepts lowercase as a different extension token; we only route QUERY.
        let lower = Method::from_bytes(b"query").unwrap();
        assert!(!is_query_method(&lower));
        let router = Router::new(vec![route(method_query(), "/search", "search_query")]);
        assert!(
            router.route(lower, "/search").is_none(),
            "lowercase query must not match QUERY route"
        );
    }

    #[test]
    fn query_method_negative_n4_cors_custom_without_query_denies() {
        let mw = CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://app.example"])
            .allowed_methods(&[Method::GET, Method::POST])
            .build()
            .unwrap();
        let req = options_req("https://app.example", "QUERY");
        let resp = mw.before(&req).expect("denied preflight still returns");
        assert_eq!(resp.status, 403);
    }

    #[test]
    fn query_method_negative_n5_preflight_false_allow_forbidden() {
        let mw = CorsMiddlewareBuilder::new()
            .allowed_origins(&["https://app.example"])
            .allowed_methods(&[Method::GET, Method::OPTIONS])
            .build()
            .unwrap();
        let req = options_req("https://app.example", "QUERY");
        let resp = mw.before(&req).unwrap();
        assert_eq!(resp.status, 403);
    }

    #[test]
    fn query_method_negative_n6_garbage_method_parse() {
        assert!(Method::from_bytes(b"QUE RY").is_err());
        assert!(Method::from_bytes(b"QUERY\n").is_err());
        assert!(Method::from_bytes(b"").is_err());
    }

    #[test]
    fn query_method_negative_n7_illegal_request_target_still_routable_separately() {
        // Router only sees path; illegal targets are Epic 10 parse/composition.
        let router = Router::new(vec![route(method_query(), "/search", "search_query")]);
        assert!(router.route(method_query(), "/search").is_some());
    }

    #[test]
    fn query_method_negative_n8_cors_header_build_no_panic() {
        let mw = CorsMiddleware::permissive();
        let _ = mw.before(&options_req("https://app.example", "QUERY"));
        let _ = mw.before(&options_req("https://app.example", "GET"));
    }

    // --- Story 11.3 P6 / retry classifier ---

    #[test]
    fn query_retry_positive_p6_query_allows_automatic_retry() {
        assert!(method_allows_automatic_retry(&method_query()));
        assert!(method_allows_automatic_retry(&Method::GET));
        assert!(method_allows_automatic_retry(&Method::HEAD));
    }

    #[test]
    fn query_retry_negative_post_patch_not_auto_retry() {
        assert!(!method_allows_automatic_retry(&Method::POST));
        assert!(!method_allows_automatic_retry(&Method::PATCH));
        assert!(!method_allows_automatic_retry(&Method::PUT));
        assert!(!method_allows_automatic_retry(&Method::DELETE));
    }
}
