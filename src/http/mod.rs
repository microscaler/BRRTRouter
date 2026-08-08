//! Coroutine-compatible outbound HTTP for security providers and internal fetch paths.
//!
//! Uses [`may_minihttp::client::HttpClient`] for HTTP and rustls-backed HTTPS.
//! Replaces `reqwest::blocking` in the request hot path so fetches run on `may::net::TcpStream`
//! without a separate tokio runtime.

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
    proxy_error_title, proxy_untyped, query_params_match_raw, raw_query_is_wire_safe,
    resolve_downstream_target, resolve_path_only, resolve_path_template,
    skip_forward_request_header, skip_forward_response_header, ProxyError, ProxyPathReason,
    ProxyTransportKind,
};
pub use uri_encode::{encode_path_segment, encode_query_component};
pub use webhook_delivery::{
    deliver_webhook, sign_body_hmac_sha256, HmacSecret, WebhookDeliveryError,
    WebhookDeliveryOptions, WebhookDeliveryResult, WebhookHmac, DEFAULT_HMAC_HEADER,
    DEFAULT_MAX_REQUEST_BODY_BYTES, DEFAULT_MAX_RESPONSE_BODY_BYTES, IDEMPOTENCY_KEY_HEADER,
};
