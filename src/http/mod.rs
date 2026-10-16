//! Coroutine-compatible outbound HTTP for security providers and internal fetch paths.
//!
//! Uses [`may_minihttp::client::HttpClient`] for HTTP and rustls-backed HTTPS.
//! Replaces `reqwest::blocking` in the request hot path so fetches run on `may::net::TcpStream`
//! without a separate tokio runtime.
//!
//! Every outbound request carries exactly one `Host`: [`fetch_get`] and friends take it from
//! the URL authority (`HttpClient::from_url`), and the BFF proxy sets the downstream Service's
//! name with [`host_header_value`] because it connects by address. See
//! `llmwiki/topics/native-http-client-and-tls.md`.

mod accept_query;
mod fetch;
pub mod method_ext;
pub mod openapi_query;
pub mod problem;
mod proxy;
pub mod uri_encode;
mod webhook_delivery;

pub use accept_query::{format_accept_query, parse_accept_query, ACCEPT_QUERY_HEADER};
pub use method_ext::{is_query_method, method_allows_automatic_retry, method_query};

pub use fetch::{
    fetch_delete, fetch_get, fetch_get_full, fetch_get_text_with_retry, fetch_patch, fetch_post,
    fetch_query, HttpFetchError, HttpFetchOptions, HttpGetResponse,
};
pub use openapi_query::{
    encode_query_form_explode, encode_query_form_no_explode, encode_query_styled,
    query_rebuild_style, QueryRebuildStyle, QueryStyleError,
};
pub use problem::{
    body_too_large_problem, legacy_error_json_enabled, multipart_problem,
    parameter_validation_problem, problem_from_legacy_body, write_problem, Problem,
    LEGACY_ERROR_JSON_ENV, PROBLEM_CONTENT_TYPE, TYPE_BASE, TYPE_MULTIPART_MISSING_BOUNDARY,
    TYPE_PARAMETER_VALIDATION_FAILED, TYPE_REQUEST_BODY_TOO_LARGE,
};
pub use proxy::{
    classify_transport_error, client_pool_key, downstream_host, downstream_http_port,
    encode_query_string, proxy_error_http_status, proxy_error_reason_code, proxy_error_response,
    proxy_error_title, proxy_sse, proxy_untyped, query_params_match_raw, raw_query_is_wire_safe,
    resolve_downstream_target, resolve_path_only, resolve_path_template,
    skip_forward_request_header, skip_forward_response_header, ProxyError, ProxyPathReason,
    ProxyTransportKind, PROXY_SSE_IDLE_ENV,
};
pub use uri_encode::{encode_path_segment, encode_query_component};
pub use webhook_delivery::{
    deliver_webhook, sign_body_hmac_sha256, HmacSecret, WebhookDeliveryError,
    WebhookDeliveryOptions, WebhookDeliveryResult, WebhookHmac, DEFAULT_HMAC_HEADER,
    DEFAULT_MAX_REQUEST_BODY_BYTES, DEFAULT_MAX_RESPONSE_BODY_BYTES, IDEMPOTENCY_KEY_HEADER,
};

/// The `Host` header value for an HTTP/1.1 request to `host` on `port`.
///
/// HTTP/1.1 requires every request to carry exactly one `Host`, and strict servers (Go's
/// `net/http`, nginx, Envoy, Google front ends) answer `400` without it. `may_minihttp` only
/// adds `Host` on its own when the client was built with `HttpClient::from_url`
/// or the request target is an absolute URI. A client built with `HttpClient::connect(addr)`
/// that sends origin-form targets (`/path?query`), as the BFF proxy does, must set it itself.
///
/// # Arguments
///
/// * `host` - DNS name or IP literal of the server. A bare IPv6 literal is bracketed;
///   one that is already bracketed is kept as is.
/// * `port` - TCP port the request is sent to.
/// * `default_port` - the scheme's default port (`80` for `http`, `443` for `https`).
///
/// # Returns
///
/// `host` when `port == default_port`, otherwise `host:port` (RFC 9110 §7.2).
///
/// # Example
///
/// ```
/// use brrtrouter::http::host_header_value;
///
/// assert_eq!(host_header_value("orders.shop.svc.cluster.local", 8080, 80), "orders.shop.svc.cluster.local:8080");
/// assert_eq!(host_header_value("api.example.com", 443, 443), "api.example.com");
/// assert_eq!(host_header_value("::1", 8080, 80), "[::1]:8080");
/// ```
#[must_use]
pub fn host_header_value(host: &str, port: u16, default_port: u16) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    if port == default_port {
        host
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod host_header_tests {
    //! Unit tests for [`super::host_header_value`].
    //!
    //! Coverage: the port is omitted only when it equals the scheme default (for both `http`
    //! and `https` defaults, and a default port used with the other scheme keeps it), DNS
    //! names and IPv4 literals pass through, and IPv6 literals are bracketed exactly once.
    //! The wire-level behaviour (one `Host` reaching a downstream) is covered by
    //! `http::proxy::tests::proxy_sends_the_downstream_host_*` and `tests/http_fetch_tests.rs`.

    use super::host_header_value;

    #[test]
    fn omits_the_http_default_port() {
        assert_eq!(host_header_value("svc.ns.svc", 80, 80), "svc.ns.svc");
    }

    #[test]
    fn omits_the_https_default_port() {
        assert_eq!(
            host_header_value("api.example.com", 443, 443),
            "api.example.com"
        );
    }

    #[test]
    fn keeps_a_non_default_port() {
        assert_eq!(host_header_value("svc.ns.svc", 8080, 80), "svc.ns.svc:8080");
    }

    #[test]
    fn keeps_the_other_schemes_default_port() {
        assert_eq!(
            host_header_value("api.example.com", 80, 443),
            "api.example.com:80"
        );
        assert_eq!(
            host_header_value("api.example.com", 443, 80),
            "api.example.com:443"
        );
    }

    #[test]
    fn passes_ipv4_literals_through() {
        assert_eq!(host_header_value("127.0.0.1", 9000, 80), "127.0.0.1:9000");
        assert_eq!(host_header_value("10.0.0.7", 80, 80), "10.0.0.7");
    }

    #[test]
    fn brackets_ipv6_literals_once() {
        assert_eq!(host_header_value("::1", 8080, 80), "[::1]:8080");
        assert_eq!(host_header_value("fd00::7", 80, 80), "[fd00::7]");
        assert_eq!(host_header_value("[::1]", 443, 443), "[::1]");
        assert_eq!(host_header_value("[::1]", 8443, 443), "[::1]:8443");
    }
}
