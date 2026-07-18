//! Coroutine-compatible outbound HTTP for security providers and internal fetch paths.
//!
//! Uses [`may_minihttp::client::HttpClient`] for HTTP and rustls-backed HTTPS.
//! Replaces `reqwest::blocking` in the request hot path so fetches run on `may::net::TcpStream`
//! without a separate tokio runtime.

mod fetch;
mod proxy;

pub use fetch::{
    fetch_get, fetch_get_full, fetch_get_text_with_retry, fetch_post, HttpFetchError,
    HttpFetchOptions, HttpGetResponse,
};
pub use proxy::{
    client_pool_key, downstream_host, downstream_http_port, proxy_untyped, resolve_path_template,
    skip_forward_request_header, skip_forward_response_header, ProxyError,
};
