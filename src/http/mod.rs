//! Coroutine-compatible outbound HTTP for security providers and internal fetch paths.
//!
//! Uses [`may_minihttp::client::HttpClient`] for HTTP and rustls-backed HTTPS.
//! Replaces `reqwest::blocking` in the request hot path so fetches run on `may::net::TcpStream`
//! without a separate tokio runtime.

mod fetch;
mod proxy;
pub mod uri_encode;

pub use fetch::{
    fetch_delete, fetch_get, fetch_get_full, fetch_get_text_with_retry, fetch_patch, fetch_post,
    HttpFetchError, HttpFetchOptions, HttpGetResponse,
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
