//! Coroutine-compatible outbound HTTP for security providers and internal fetch paths.
//!
//! Uses [`may_http::client::HttpClient`] for plain HTTP and a minimal rustls stack for HTTPS.
//! Replaces `reqwest::blocking` in the request hot path so fetches run on `may::net::TcpStream`
//! without a separate tokio runtime.

mod fetch;

pub use fetch::{fetch_get, fetch_get_text_with_retry, fetch_post, HttpFetchError, HttpFetchOptions};
