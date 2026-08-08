//! Outbound webhook delivery kit (Story 12.5 / [#396](https://github.com/microscaler/BRRTRouter/issues/396)).
//!
//! Small platform helper for Sesame-style delivery: HTTP POST of JSON (or arbitrary bytes)
//! with optional HMAC-SHA256 signature header, bounded retries, and backoff.
//!
//! This is **not** OpenAPI Callback Object auto-fire (parked). Subscription CRUD remains
//! ordinary OpenAPI paths.

use std::fmt;
use std::time::Duration;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::warn;
use url::Url;

use crate::http::{fetch_post, HttpFetchError, HttpFetchOptions};

type HmacSha256 = Hmac<Sha256>;

/// Default max outbound request body (1 MiB).
pub const DEFAULT_MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;

/// Default max response body to buffer from the subscriber.
pub const DEFAULT_MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

/// Default header for HMAC-SHA256 signatures (`sha256=<hex>`).
pub const DEFAULT_HMAC_HEADER: &str = "X-Hub-Signature-256";

/// Default `Idempotency-Key` header name.
pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

/// HMAC signing secret — never printed via [`Display`] / [`Debug`].
#[derive(Clone)]
pub struct HmacSecret(String);

impl HmacSecret {
    /// Wrap a raw secret string.
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// Borrow the raw secret for signing only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// `true` when the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for HmacSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HmacSecret([REDACTED])")
    }
}

impl fmt::Display for HmacSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Optional HMAC-SHA256 signing for the request body.
#[derive(Debug, Clone)]
pub struct WebhookHmac {
    /// Shared secret (redacted in logs).
    pub secret: HmacSecret,
    /// Header that receives `sha256=<hex>`.
    pub header_name: String,
    /// When `true`, an empty secret is a hard error ([`WebhookDeliveryError::EmptyHmacSecret`]).
    /// When `false`, an empty secret skips signing.
    pub required: bool,
}

impl WebhookHmac {
    /// Required HMAC with [`DEFAULT_HMAC_HEADER`].
    #[must_use]
    pub fn required(secret: impl Into<String>) -> Self {
        Self {
            secret: HmacSecret::new(secret),
            header_name: DEFAULT_HMAC_HEADER.to_string(),
            required: true,
        }
    }

    /// Optional HMAC (skipped when secret empty).
    #[must_use]
    pub fn optional(secret: impl Into<String>) -> Self {
        Self {
            secret: HmacSecret::new(secret),
            header_name: DEFAULT_HMAC_HEADER.to_string(),
            required: false,
        }
    }
}

/// Options for a single outbound webhook delivery.
#[derive(Debug, Clone)]
pub struct WebhookDeliveryOptions {
    /// Subscriber callback URL (`http` / `https`).
    pub url: String,
    /// Request body bytes (typically JSON).
    pub body: Vec<u8>,
    /// Extra headers (name, value). Applied before HMAC / Idempotency-Key.
    pub headers: Vec<(String, String)>,
    /// Optional HMAC signing.
    pub hmac: Option<WebhookHmac>,
    /// Optional idempotency key (forwarded as [`IDEMPOTENCY_KEY_HEADER`]).
    pub idempotency_key: Option<String>,
    /// Total attempts including the first (minimum 1).
    pub max_attempts: u32,
    /// Backoff before retry attempt `n` (`initial_backoff * 2^(n-1)`).
    pub initial_backoff: Duration,
    /// Per-attempt client timeout ([`HttpFetchOptions::timeout`]).
    pub timeout: Duration,
    /// Max response body octets to read.
    pub max_response_body_bytes: usize,
    /// Max request body octets; larger bodies fail before connect.
    pub max_request_body_bytes: usize,
    /// When set, sent as `Content-Type` (default `application/json`).
    pub content_type: Option<String>,
}

impl Default for WebhookDeliveryOptions {
    fn default() -> Self {
        Self {
            url: String::new(),
            body: Vec::new(),
            headers: Vec::new(),
            hmac: None,
            idempotency_key: None,
            max_attempts: 3,
            initial_backoff: Duration::from_millis(50),
            timeout: Duration::from_secs(5),
            max_response_body_bytes: DEFAULT_MAX_RESPONSE_BODY_BYTES,
            max_request_body_bytes: DEFAULT_MAX_REQUEST_BODY_BYTES,
            content_type: Some("application/json".to_string()),
        }
    }
}

/// Successful delivery (2xx).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookDeliveryResult {
    /// HTTP status from the subscriber.
    pub status: u16,
    /// Response body (bounded).
    pub response_body: Vec<u8>,
    /// Attempts used (1 = first try succeeded).
    pub attempts: u32,
}

/// Delivery failure (never panics; secrets never appear in [`Display`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookDeliveryError {
    /// URL parse / scheme rejection.
    InvalidUrl(String),
    /// HMAC required but secret empty.
    EmptyHmacSecret,
    /// Request body larger than [`WebhookDeliveryOptions::max_request_body_bytes`].
    RequestBodyTooLarge { len: usize, max: usize },
    /// Transport / TLS / connect / read failure from the fetch layer.
    Transport(HttpFetchError),
    /// Non-success HTTP status after retries exhausted (or non-retryable 4xx).
    HttpStatus {
        status: u16,
        attempts: u32,
        body: Vec<u8>,
    },
}

impl fmt::Display for WebhookDeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(msg) => write!(f, "invalid webhook URL: {msg}"),
            Self::EmptyHmacSecret => write!(f, "HMAC secret required but empty"),
            Self::RequestBodyTooLarge { len, max } => {
                write!(f, "webhook body {len} bytes exceeds limit {max}")
            }
            Self::Transport(err) => write!(f, "webhook transport: {err}"),
            Self::HttpStatus {
                status, attempts, ..
            } => write!(f, "webhook HTTP {status} after {attempts} attempt(s)"),
        }
    }
}

impl std::error::Error for WebhookDeliveryError {}

/// Sign `body` with HMAC-SHA256; return `sha256=<hex>` (lowercase hex).
#[must_use]
pub fn sign_body_hmac_sha256(secret: &str, body: &[u8]) -> String {
    use hmac::KeyInit;
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC-SHA256 accepts any key length");
    mac.update(body);
    let tag = mac.finalize().into_bytes();
    format!("sha256={}", hex_encode(&tag))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn validate_url(url: &str) -> Result<(), WebhookDeliveryError> {
    let parsed = Url::parse(url).map_err(|e| WebhookDeliveryError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        other => Err(WebhookDeliveryError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
    }
}

fn retryable_status(status: u16) -> bool {
    status == 408 || status == 429 || (500..600).contains(&status)
}

fn backoff_for_attempt(initial: Duration, attempt_index: u32) -> Duration {
    // attempt_index 0 = first retry after failure → 1× initial
    let shift = attempt_index.min(8);
    initial.saturating_mul(1u32 << shift)
}

/// Deliver a webhook POST with optional HMAC, retries, and backoff.
///
/// # Errors
///
/// Returns [`WebhookDeliveryError`] on validation, transport, or non-2xx outcomes.
/// 4xx responses are **not** retried (surfaced immediately). 5xx / selected
/// transient statuses and transport errors retry up to `max_attempts`.
pub fn deliver_webhook(
    options: &WebhookDeliveryOptions,
) -> Result<WebhookDeliveryResult, WebhookDeliveryError> {
    validate_url(&options.url)?;

    if options.body.len() > options.max_request_body_bytes {
        return Err(WebhookDeliveryError::RequestBodyTooLarge {
            len: options.body.len(),
            max: options.max_request_body_bytes,
        });
    }

    let max_attempts = options.max_attempts.max(1);
    let mut headers = options.headers.clone();

    if let Some(ct) = &options.content_type {
        if !headers
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        {
            headers.push(("Content-Type".to_string(), ct.clone()));
        }
    }

    if let Some(key) = &options.idempotency_key {
        headers.push((IDEMPOTENCY_KEY_HEADER.to_string(), key.clone()));
    }

    if let Some(hmac_cfg) = &options.hmac {
        if hmac_cfg.secret.is_empty() {
            if hmac_cfg.required {
                return Err(WebhookDeliveryError::EmptyHmacSecret);
            }
        } else {
            let sig = sign_body_hmac_sha256(hmac_cfg.secret.expose(), &options.body);
            headers.push((hmac_cfg.header_name.clone(), sig));
        }
    }

    let fetch_opts = HttpFetchOptions {
        timeout: options.timeout,
        max_body_bytes: options.max_response_body_bytes,
        extra_headers: headers,
    };

    let mut last_status: Option<(u16, Vec<u8>)> = None;
    let mut last_transport: Option<HttpFetchError> = None;

    for attempt in 0..max_attempts {
        let attempt_no = attempt + 1;
        match fetch_post(&options.url, &options.body, &fetch_opts) {
            Ok((status, body)) if (200..300).contains(&status) => {
                return Ok(WebhookDeliveryResult {
                    status,
                    response_body: body,
                    attempts: attempt_no,
                });
            }
            Ok((status, body)) => {
                last_status = Some((status, body));
                last_transport = None;
                if !retryable_status(status) || attempt_no >= max_attempts {
                    let (status, body) = last_status.take().unwrap_or((status, Vec::new()));
                    return Err(WebhookDeliveryError::HttpStatus {
                        status,
                        attempts: attempt_no,
                        body,
                    });
                }
                warn!(
                    url = %options.url,
                    status,
                    attempt = attempt_no,
                    max_attempts,
                    "webhook delivery got retryable status; backing off"
                );
            }
            Err(err) => {
                last_transport = Some(err.clone());
                last_status = None;
                if attempt_no >= max_attempts {
                    return Err(WebhookDeliveryError::Transport(err));
                }
                warn!(
                    url = %options.url,
                    attempt = attempt_no,
                    max_attempts,
                    error = %err,
                    "webhook delivery transport error; backing off"
                );
            }
        }

        let sleep_for = backoff_for_attempt(options.initial_backoff, attempt);
        if !sleep_for.is_zero() {
            may::coroutine::sleep(sleep_for);
        }
    }

    if let Some((status, body)) = last_status {
        Err(WebhookDeliveryError::HttpStatus {
            status,
            attempts: max_attempts,
            body,
        })
    } else {
        Err(WebhookDeliveryError::Transport(
            last_transport.unwrap_or_else(|| HttpFetchError::Request("no attempts".into())),
        ))
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn webhook_hmac_p2_sign_stable_hex() {
        let sig = sign_body_hmac_sha256("secret", b"{\"a\":1}");
        assert!(sig.starts_with("sha256="));
        assert_eq!(sig.len(), "sha256=".len() + 64);
        // Cross-check with a second call (determinism).
        assert_eq!(sig, sign_body_hmac_sha256("secret", b"{\"a\":1}"));
    }

    #[test]
    fn webhook_n3_invalid_url() {
        let mut opts = WebhookDeliveryOptions::default();
        opts.url = "not a url".into();
        opts.body = b"{}".to_vec();
        let err = deliver_webhook(&opts).unwrap_err();
        assert!(matches!(err, WebhookDeliveryError::InvalidUrl(_)));
    }

    #[test]
    fn webhook_n4_empty_secret_required() {
        let mut opts = WebhookDeliveryOptions::default();
        opts.url = "http://127.0.0.1:9/hook".into();
        opts.body = b"{}".to_vec();
        opts.hmac = Some(WebhookHmac::required(""));
        let err = deliver_webhook(&opts).unwrap_err();
        assert_eq!(err, WebhookDeliveryError::EmptyHmacSecret);
    }

    #[test]
    fn webhook_n5_oversized_body() {
        let mut opts = WebhookDeliveryOptions::default();
        opts.url = "http://127.0.0.1:9/hook".into();
        opts.body = vec![0u8; 100];
        opts.max_request_body_bytes = 10;
        let err = deliver_webhook(&opts).unwrap_err();
        assert!(matches!(
            err,
            WebhookDeliveryError::RequestBodyTooLarge { len: 100, max: 10 }
        ));
    }

    #[test]
    fn webhook_n8_secret_not_in_debug_or_display() {
        let secret = HmacSecret::new("super-secret-value-do-not-leak");
        let dbg = format!("{secret:?}");
        let disp = format!("{secret}");
        assert!(!dbg.contains("super-secret"));
        assert!(!disp.contains("super-secret"));
        assert!(dbg.contains("REDACTED"));
        let err = WebhookDeliveryError::EmptyHmacSecret;
        assert!(!format!("{err}").contains("super-secret"));
    }

    #[test]
    fn webhook_n4_optional_empty_secret_skips_hmac() {
        let hmac = WebhookHmac::optional("");
        assert!(hmac.secret.is_empty());
        assert!(!hmac.required);
    }
}
