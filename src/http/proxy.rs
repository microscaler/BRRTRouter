//! BFF downstream HTTP proxy — Kubernetes Service-name routing.
//!
//! Replaces generated inline proxy logic in `templates/controller.rs.txt`.
//! Downstream targets are resolved by OpenAPI `x-service` (Kubernetes Service name)
//! and `SERVICE_HTTP_PORT` (default 8080).
//!
//! # Connections
//!
//! Proxied request/response calls ([`proxy_untyped`]) reuse kept-alive connections from a pool
//! keyed by downstream origin (Service host and port), so one Service's connections are never
//! used for another (FR-26 still holds: no connection crosses services). Before, every proxied
//! call opened a new TCP connection. Event streams ([`proxy_sse`]) hold their connection for the
//! life of the stream and still open one each. Tuning, all optional:
//!
//! | variable | default | meaning |
//! |---|---|---|
//! | `BRRTR_PROXY_POOL` | `on` | `off`: a new connection per proxied call (the old behaviour) |
//! | `BRRTR_PROXY_POOL_MAX` | 1024 | connections, all downstreams |
//! | `BRRTR_PROXY_POOL_MAX_PER_HOST` | 256 | connections per downstream; a call waits past it |
//! | `BRRTR_PROXY_POOL_IDLE_SECS` | 30 | an idle connection is closed after this |
//! | `BRRTR_PROXY_POOL_LIFETIME_SECS` | 60 | no connection is reused past this age |
//!
//! kube-proxy balances a ClusterIP Service per *connection*: the lifetime bound is what spreads a
//! busy BFF's calls over the downstream's pods again, and what moves them off pods that left.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use http_legacy::{HeaderMap, HeaderName, HeaderValue, Method, Uri};
use may_minihttp::client::{
    Client, ClientError, ClientErrorKind, HttpClient, RedirectPolicy, Resolver,
    Response as DownstreamResponse, StreamingResponse,
};
use serde_json::Value;

use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use crate::router::ParamVec;
use crate::server::request::parse_query_params;
use crate::server::request_target::{max_request_target_octets, request_target_exceeds_limit};

const DEFAULT_DOWNSTREAM_PORT: u16 = 8080;
const DEFAULT_PROXY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PROXY_BODY_BYTES: usize = 16 * 1024 * 1024;

/// The most a downstream's `Content-Length` reserves before any body arrives; a larger body grows
/// past it as it is read. Bounds what an exaggerated header can make many responses hold at once.
const MAX_PROXY_BODY_PREALLOC: usize = 1024 * 1024;

/// Env: how long a proxied event stream may stay silent before the relay ends
/// it (seconds, default 300). The browser's `EventSource` then reconnects with
/// `Last-Event-ID`. Downstream keepalive comments reset the clock.
pub const PROXY_SSE_IDLE_ENV: &str = "BRRTR_PROXY_SSE_IDLE_SECS";
const DEFAULT_PROXY_SSE_IDLE: Duration = Duration::from_secs(300);
/// A downstream that never ends a frame stops being relayed past this size.
const MAX_SSE_FRAME_BYTES: usize = 1024 * 1024;
/// Most payload a relayed stream may queue for a client that reads slowly; past
/// it (or past `BRRTR_SSE_QUEUE_BOUND` frames) the relay ends and the client
/// reconnects from its last event id.
const MAX_SSE_QUEUED_BYTES: usize = 4 * 1024 * 1024;

/// Composition failure reason for rebuilt request-targets (Story 10.7).
///
/// Every `InvalidPath` carries one of these — catch-all string-only path errors
/// are forbidden so ops can grep stable `reason` codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProxyPathReason {
    /// Rebuilt target failed `http::Uri` parse (Loadlinker-class).
    InvalidUri,
    /// Path template still contains `{param}` after substitution.
    UnresolvedPathParam,
    /// Resolved path introduced a raw `?` (smuggling / misconfig).
    PathContainsQuestion,
    /// Passthrough raw query contained space/CTL/`#`.
    UnsafeRawQuery,
    /// OpenAPI query style/explode not supported for proxy rebuild (Story 10.9).
    UnsupportedQueryStyle,
}

impl ProxyPathReason {
    /// Stable machine code for metrics / JSON `reason`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidUri => "invalid_uri",
            Self::UnresolvedPathParam => "unresolved_path_param",
            Self::PathContainsQuestion => "path_contains_question",
            Self::UnsafeRawQuery => "unsafe_raw_query",
            Self::UnsupportedQueryStyle => "unsupported_query_style",
        }
    }

    /// Stable `invalid path: …` suffix for ops grep (no full URI).
    #[must_use]
    pub const fn display_detail(self) -> &'static str {
        match self {
            Self::InvalidUri => "invalid uri character",
            Self::UnresolvedPathParam => "unresolved path template placeholder",
            Self::PathContainsQuestion => "resolved path must not contain '?'",
            Self::UnsafeRawQuery => "raw query contains unsafe octets for passthrough",
            Self::UnsupportedQueryStyle => "unsupported OpenAPI query style for proxy rebuild",
        }
    }
}

/// Errors from the BFF downstream proxy layer.
///
/// # Status taxonomy (Story 10.7)
///
/// | Class | Variants | HTTP |
/// |-------|----------|------|
/// | Composition | [`InvalidPath`], [`InvalidMethod`], [`RequestTargetTooLong`] | 400 / 414 |
/// | Gateway timeout | [`Timeout`] | 504 |
/// | Upstream / transport | [`Dns`], [`Connect`], [`Request`], [`Response`], [`BodyTooLarge`] | 502 |
/// | Internal | [`BodySerialize`] | 500 |
///
/// Use [`proxy_error_http_status`] / [`proxy_error_response`] — never map all
/// errors to 502.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyError {
    InvalidMethod,
    /// URI composition failure — always carries a [`ProxyPathReason`].
    InvalidPath {
        reason: ProxyPathReason,
    },
    /// Outbound or inbound request-target exceeds configured max (Story 10.6 → 414).
    RequestTargetTooLong {
        len: usize,
        max: usize,
    },
    Dns,
    Connect,
    /// Dial or request deadline exceeded (when distinguishable from peer reset).
    Timeout,
    Request,
    Response,
    BodySerialize,
    BodyTooLarge,
}

impl ProxyError {
    #[must_use]
    pub const fn invalid_path(reason: ProxyPathReason) -> Self {
        Self::InvalidPath { reason }
    }
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMethod => write!(f, "invalid method"),
            Self::InvalidPath { reason } => {
                write!(f, "invalid path: {}", reason.display_detail())
            }
            Self::RequestTargetTooLong { len, max } => {
                write!(f, "request-target too long: {len} > {max}")
            }
            Self::Dns => write!(f, "dns: resolution failed"),
            Self::Connect => write!(f, "connect: connection failed"),
            Self::Timeout => write!(f, "timeout: gateway timeout"),
            Self::Request => write!(f, "request: request failed"),
            Self::Response => write!(f, "response: response failed"),
            Self::BodySerialize => write!(f, "body serialize: failed"),
            Self::BodyTooLarge => write!(f, "response body exceeds limit"),
        }
    }
}

impl std::error::Error for ProxyError {}

/// HTTP status for a [`ProxyError`] (Story 10.7 taxonomy).
#[must_use]
pub fn proxy_error_http_status(err: &ProxyError) -> u16 {
    match err {
        ProxyError::InvalidMethod | ProxyError::InvalidPath { .. } => 400,
        ProxyError::RequestTargetTooLong { .. } => 414,
        ProxyError::Timeout => 504,
        ProxyError::BodySerialize => 500,
        ProxyError::Dns
        | ProxyError::Connect
        | ProxyError::Request
        | ProxyError::Response
        | ProxyError::BodyTooLarge => 502,
    }
}

/// Stable reason code for metrics / JSON (`reason` field).
#[must_use]
pub fn proxy_error_reason_code(err: &ProxyError) -> &'static str {
    match err {
        ProxyError::InvalidMethod => "invalid_method",
        ProxyError::InvalidPath { reason } => reason.code(),
        ProxyError::RequestTargetTooLong { .. } => "request_target_too_long",
        ProxyError::Dns => "dns",
        ProxyError::Connect => "connect",
        ProxyError::Timeout => "timeout",
        ProxyError::Request => "request",
        ProxyError::Response => "response",
        ProxyError::BodySerialize => "body_serialize",
        ProxyError::BodyTooLarge => "body_too_large",
    }
}

/// Title string for the JSON `error` field (stable for ops).
#[must_use]
pub fn proxy_error_title(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        414 => "URI Too Long",
        500 => "Internal Server Error",
        504 => "Gateway Timeout",
        _ => "Bad Gateway",
    }
}

/// Map a proxy error to a client-facing [`HandlerResponse`].
#[must_use]
pub fn proxy_error_response(err: &ProxyError) -> HandlerResponse {
    let status = proxy_error_http_status(err);
    let mut resp = HandlerResponse::json(
        status,
        serde_json::json!({
            "error": proxy_error_title(status),
            "reason": proxy_error_reason_code(err),
            "message": err.to_string(),
        }),
    );
    resp.headers
        .push((Arc::from("content-type"), "application/json".to_string()));
    resp
}

/// Classify I/O / client errors; timeouts → [`ProxyError::Timeout`] (→ 504).
#[must_use]
pub fn classify_transport_error(kind: ProxyTransportKind, message: &str) -> ProxyError {
    if looks_like_timeout(message) {
        return ProxyError::Timeout;
    }
    match kind {
        ProxyTransportKind::Dns => ProxyError::Dns,
        ProxyTransportKind::Connect => ProxyError::Connect,
        ProxyTransportKind::Request => ProxyError::Request,
        ProxyTransportKind::Response => ProxyError::Response,
    }
}

/// Transport stage for [`classify_transport_error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyTransportKind {
    Dns,
    Connect,
    Request,
    Response,
}

fn looks_like_timeout(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline exceeded")
        || lower.contains("time out")
}

/// Substitute `{param}` placeholders in a path template (no query).
#[must_use]
pub fn resolve_path_only(path_template: &str, path_params: &ParamVec) -> String {
    use crate::http::uri_encode::encode_path_segment;

    let mut resolved_path = path_template.to_string();
    for (k, v) in path_params {
        let needle = format!("{{{k}}}");
        // Path segments: encode so spaces/unicode stay URI-safe; `/` in a value
        // becomes `%2F` (OpenAPI path params are single segments).
        resolved_path = resolved_path.replace(&needle, encode_path_segment(v.as_ref()).as_ref());
    }
    resolved_path
}

/// Encode query params as OpenAPI `form` + `explode=true` (`?k=v&…`).
///
/// Empty → `""` (no bare `?`). For `explode=false` or style gates see
/// [`crate::http::openapi_query`].
#[must_use]
pub fn encode_query_string(query_params: &ParamVec) -> String {
    crate::http::openapi_query::encode_query_form_explode(query_params)
}

/// `true` when `raw` query octets are URI-safe for passthrough (no space/CTL/`#`).
#[must_use]
pub fn raw_query_is_wire_safe(raw: &str) -> bool {
    !raw.bytes().any(|b| b <= 0x20 || b == b'#' || b == 0x7f)
}

/// `true` when parsed `raw` matches `query_params` (order + values) — middleware unmutated.
#[must_use]
pub fn query_params_match_raw(raw: &str, query_params: &ParamVec) -> bool {
    let parsed = parse_query_params(&format!("/?{raw}"));
    if parsed.len() != query_params.len() {
        return false;
    }
    parsed
        .iter()
        .zip(query_params.iter())
        .all(|((k1, v1), (k2, v2))| k1.as_ref() == k2.as_ref() && v1 == v2)
}

/// Resolve downstream request-target with optional query passthrough (Story 10.5)
/// and length enforcement (Story 10.6).
///
/// Passthrough applies when `raw_query` is present, wire-safe, and still matches
/// `query_params` (no middleware mutation). Path templates with `{param}` still
/// substitute via 10.4 encoders; query octets are preserved when eligible.
pub fn resolve_downstream_target(
    path_template: &str,
    path_params: &ParamVec,
    query_params: &ParamVec,
    raw_query: Option<&str>,
) -> Result<String, ProxyError> {
    let path = resolve_path_only(path_template, path_params);
    if path.contains('{') {
        return Err(ProxyError::invalid_path(
            ProxyPathReason::UnresolvedPathParam,
        ));
    }
    // N8: path must not introduce a second `?` (smuggling); encoders percent-encode `?`.
    if path.contains('?') {
        return Err(ProxyError::invalid_path(
            ProxyPathReason::PathContainsQuestion,
        ));
    }

    let query_suffix = match raw_query {
        Some(raw) if query_params_match_raw(raw, query_params) => {
            if raw.is_empty() || parse_query_params(&format!("/?{raw}")).is_empty() {
                // No pairs (incl. trailing `?` alone) → no spurious `?` on wire.
                String::new()
            } else if !raw_query_is_wire_safe(raw) {
                return Err(ProxyError::invalid_path(ProxyPathReason::UnsafeRawQuery));
            } else {
                format!("?{raw}")
            }
        }
        _ => encode_query_string(query_params),
    };

    let target = format!("{path}{query_suffix}");
    let max = max_request_target_octets();
    if request_target_exceeds_limit(&target, max) {
        tracing::debug!(
            target_len = target.len(),
            max_len = max,
            "Outbound request-target exceeds configured max; rejecting with 414"
        );
        return Err(ProxyError::RequestTargetTooLong {
            len: target.len(),
            max,
        });
    }
    Ok(target)
}

/// Resolve `{param}` placeholders and append query string (always rebuild).
///
/// Incoming `HandlerRequest` params are already decoded. Rebuild uses
/// [`crate::http::uri_encode`] so spaces become `%20` (never `+`) and reserved
/// characters cannot corrupt the request-target (provinces 502 regression).
/// Prefer [`resolve_downstream_target`] when `raw_query` is available.
#[must_use]
pub fn resolve_path_template(
    path_template: &str,
    path_params: &ParamVec,
    query_params: &ParamVec,
) -> String {
    let path = resolve_path_only(path_template, path_params);
    let mut resolved = path;
    resolved.push_str(&encode_query_string(query_params));
    resolved
}

/// Kubernetes DNS host for a downstream Service in the pod namespace.
#[must_use]
pub fn downstream_host(service: &str) -> String {
    if let Ok(ns) = std::env::var("POD_NAMESPACE") {
        if !ns.is_empty() {
            return format!("{service}.{ns}.svc.cluster.local");
        }
    }
    service.to_string()
}

/// Cluster-wide downstream HTTP port (PRD: uniform 8080).
#[must_use]
pub fn downstream_http_port() -> u16 {
    std::env::var("SERVICE_HTTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_DOWNSTREAM_PORT)
}

/// How long a resolved downstream address is reused before it is looked up again
/// (`BRRTR_PROXY_DNS_TTL_SECS`, default 30; `0` resolves on every request).
///
/// A Service's ClusterIP does not change while the Service exists; a stale address (the Service
/// was recreated) fails to connect, which drops it and resolves again before giving up.
fn dns_ttl() -> Duration {
    static TTL: OnceLock<Duration> = OnceLock::new();
    *TTL.get_or_init(|| {
        Duration::from_secs(
            std::env::var("BRRTR_PROXY_DNS_TTL_SECS")
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(30),
        )
    })
}

/// Resolved downstream addresses by host: (port, address, resolved at).
type DnsCache = RwLock<HashMap<String, (u16, SocketAddr, Instant)>>;

fn dns_cache() -> &'static DnsCache {
    static CACHE: OnceLock<DnsCache> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The name handed to the resolver. A Kubernetes Service FQDN gets a trailing dot: it has four
/// dots, below the pod resolver's `ndots:5`, so without it every lookup first walks the pod's
/// search domains (`<name>.<ns>.svc.cluster.local`, `.svc.cluster.local`, ... — an NXDOMAIN
/// round trip, A and AAAA, for each) before asking for the name itself.
fn lookup_name(host: &str) -> Cow<'_, str> {
    if host.ends_with(".cluster.local") {
        Cow::Owned(format!("{host}."))
    } else {
        Cow::Borrowed(host)
    }
}

/// A downstream address and whether it came from the cache (and so may be stale).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Resolved {
    addr: SocketAddr,
    cached: bool,
}

/// The system resolver: one address for `host:port` (see [`lookup_name`]).
fn system_lookup(host: &str, port: u16) -> Result<SocketAddr, ProxyError> {
    ToSocketAddrs::to_socket_addrs(&(lookup_name(host).as_ref(), port))
        .map_err(|e| classify_transport_error(ProxyTransportKind::Dns, &e.to_string()))?
        .next()
        .ok_or(ProxyError::Dns)
}

/// The address to connect to for `host:port`: from the cache while fresh, else resolved (a
/// blocking `getaddrinfo` on the coroutine's worker thread, so not on every request).
fn resolve_downstream(host: &str, port: u16) -> Result<Resolved, ProxyError> {
    resolve_downstream_with(host, port, &system_lookup)
}

fn resolve_downstream_with(
    host: &str,
    port: u16,
    lookup: &dyn Fn(&str, u16) -> Result<SocketAddr, ProxyError>,
) -> Result<Resolved, ProxyError> {
    let ttl = dns_ttl();
    if !ttl.is_zero() {
        if let Ok(cache) = dns_cache().read() {
            if let Some((p, addr, at)) = cache.get(host) {
                if *p == port && at.elapsed() < ttl {
                    return Ok(Resolved {
                        addr: *addr,
                        cached: true,
                    });
                }
            }
        }
    }
    let addr = lookup(host, port)?;
    if !ttl.is_zero() {
        if let Ok(mut cache) = dns_cache().write() {
            cache.insert(host.to_string(), (port, addr, Instant::now()));
        }
    }
    Ok(Resolved {
        addr,
        cached: false,
    })
}

/// Drop `host`'s cached address if it is still `addr`. A request that already re-resolved may
/// have stored a fresh address; that one is kept.
fn forget_downstream(host: &str, addr: SocketAddr) {
    if let Ok(mut cache) = dns_cache().write() {
        if cache.get(host).is_some_and(|(_, a, _)| *a == addr) {
            cache.remove(host);
        }
    }
}

/// Drop `host`'s cached address, whatever it is (the pooled path does not see which address a
/// refused connect used).
fn forget_downstream_host(host: &str) {
    if let Ok(mut cache) = dns_cache().write() {
        cache.remove(host);
    }
}

fn is_timeout(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::TimedOut || looks_like_timeout(&e.to_string())
}

/// Pause before retrying a connect to a fresh address (`PROXY_CONNECT_RETRY_MS`, default 100;
/// `0` turns the retry off).
fn connect_retry_delay() -> Option<Duration> {
    let ms = std::env::var("PROXY_CONNECT_RETRY_MS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_CONNECT_RETRY_MS);
    (ms > 0).then(|| Duration::from_millis(ms.min(MAX_CONNECT_RETRY_MS)))
}

const DEFAULT_CONNECT_RETRY_MS: u64 = 100;
const MAX_CONNECT_RETRY_MS: u64 = 2_000;

/// Connect to `resolved`. If that fails and the address came from the cache it may be stale
/// (the Service was recreated): drop it (if still cached), resolve again and connect once more.
/// Every request that used a stale address retries, not only the first to fail; nothing has
/// been sent yet, so this is safe for any method.
///
/// A fresh address that refuses is retried once after a short pause
/// ([`connect_retry_delay`]): during a rollout the Service's endpoints change under it and a
/// connect can land on a pod that is going away (pp, Oct 2026: every `connect` 502 on the BFFs
/// came in rollout windows). Nothing has been sent, so this too is safe for any method.
fn connect_downstream_with<T>(
    host: &str,
    port: u16,
    resolved: Resolved,
    lookup: &dyn Fn(&str, u16) -> Result<SocketAddr, ProxyError>,
    connect: impl Fn(SocketAddr) -> std::io::Result<T>,
    retry_delay: Option<Duration>,
) -> Result<T, ProxyError> {
    let connect_error =
        |e: std::io::Error| classify_transport_error(ProxyTransportKind::Connect, &e.to_string());
    match connect(resolved.addr) {
        Ok(conn) => Ok(conn),
        Err(_) if resolved.cached => {
            forget_downstream(host, resolved.addr);
            let fresh = resolve_downstream_with(host, port, lookup)?;
            connect(fresh.addr).map_err(connect_error)
        }
        // A connect that timed out is not retried: an unreachable downstream would cost the
        // connect timeout twice before the 504 (Codex review on #521).
        Err(e) => match retry_delay.filter(|_| !is_timeout(&e)) {
            Some(delay) => {
                // Yields the coroutine; a plain thread sleeps.
                may::coroutine::sleep(delay);
                connect(resolved.addr).map_err(connect_error)
            }
            None => Err(connect_error(e)),
        },
    }
}

/// Stable key for a downstream origin `(host, port)`: the unit the proxy pools connections by.
#[must_use]
pub fn client_pool_key(host: &str, port: u16) -> String {
    format!("{host}:{port}")
}

/// Hop-by-hop / connection headers that must not be forwarded to downstream.
#[must_use]
pub fn skip_forward_request_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("host")
        || name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("content-length")
        || name.eq_ignore_ascii_case("transfer-encoding")
        || name.eq_ignore_ascii_case("upgrade")
        || name.eq_ignore_ascii_case("te")
        || name.eq_ignore_ascii_case("trailer")
        || name.eq_ignore_ascii_case("proxy-connection")
}

/// Hop-by-hop headers that must not be forwarded to the client.
#[must_use]
pub fn skip_forward_response_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("content-length")
        || name.eq_ignore_ascii_case("transfer-encoding")
        || name.eq_ignore_ascii_case("keep-alive")
        || name.eq_ignore_ascii_case("upgrade")
        || name.eq_ignore_ascii_case("trailer")
        || name.eq_ignore_ascii_case("proxy-authenticate")
        // BRRTRouter's internal body marker: only this process sets it (a downstream's would make
        // the writer base64-decode or pass through a body it should not).
        || name.eq_ignore_ascii_case(crate::server::response::RAW_BODY_ENCODING_HEADER)
}

fn proxy_timeout() -> Duration {
    std::env::var("PROXY_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| Duration::from_secs(secs.max(1)))
        .unwrap_or(DEFAULT_PROXY_TIMEOUT)
}

fn read_bounded_body(
    reader: &mut impl Read,
    max_body: usize,
    size_hint: Option<usize>,
) -> Result<Vec<u8>, ProxyError> {
    // Sized from Content-Length when the downstream sends one: no doubling reallocations (a
    // 200 KB body otherwise grows through ten buffers). The reservation is capped
    // (`MAX_PROXY_BODY_PREALLOC`): a header is not a body, and a larger one still grows as read.
    let mut buf = Vec::with_capacity(
        size_hint.map_or(0, |n| n.min(max_body).min(MAX_PROXY_BODY_PREALLOC) + 1),
    );
    reader
        .by_ref()
        .take(max_body as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| classify_transport_error(ProxyTransportKind::Response, &e.to_string()))?;
    if buf.len() > max_body {
        return Err(ProxyError::BodyTooLarge);
    }
    Ok(buf)
}

fn response_body_value(buf: &[u8], content_type: Option<&str>) -> Value {
    let looks_json = content_type
        .map(|ct| ct.contains("application/json") || ct.contains("+json"))
        .unwrap_or(false)
        || buf.first().is_some_and(|b| *b == b'{' || *b == b'[');

    if looks_json {
        if let Ok(v) = serde_json::from_slice(buf) {
            return v;
        }
    }
    if buf.is_empty() {
        return Value::Null;
    }
    Value::String(String::from_utf8_lossy(buf).into_owned())
}

/// Whether proxied 2xx JSON bodies pass through as bytes (`BRRTR_PROXY_PASSTHROUGH=on|true|1`, read once;
/// default off: parse, validate, re-serialise, as before). ADR 006 §2.
#[must_use]
pub fn passthrough_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("BRRTR_PROXY_PASSTHROUGH").is_ok_and(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "on" | "true" | "1" | "yes"
            )
        })
    })
}

/// The downstream body as JSON text to pass through untouched, or the bytes back when it is not
/// one: a non-2xx status, a Content-Type that is not JSON, an empty body or invalid UTF-8.
fn passthrough_body(
    buf: Vec<u8>,
    status: u16,
    content_type: Option<&str>,
) -> Result<String, Vec<u8>> {
    let json = content_type.is_some_and(|ct| {
        let ct = ct.to_ascii_lowercase();
        ct.contains("application/json") || ct.contains("+json")
    });
    if !(200..300).contains(&status) || !json || buf.is_empty() {
        return Err(buf);
    }
    String::from_utf8(buf).map_err(std::string::FromUtf8Error::into_bytes)
}

/// Connect to the downstream Service and send the request. The client comes
/// back with the response because it owns the connection the body is read from.
/// Send `req` downstream and read the response head. A GET or HEAD whose request failed after
/// connecting (`ProxyError::Request`: the peer reset or closed before answering, as a pod going
/// away in a rollout does) is sent once more; those methods are idempotent. A timeout is not
/// retried: the downstream may still be working on it, and a second wait doubles the latency.
fn send_downstream(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
    default_accept: &str,
    identity_encoding: bool,
) -> Result<(HttpClient, DownstreamResponse), ProxyError> {
    let first = send_downstream_once(
        req,
        downstream_service,
        path_template,
        default_accept,
        identity_encoding,
    );
    match first {
        Err(ProxyError::Request) if is_idempotent_read(req.method.as_str()) => {
            send_downstream_once(
                req,
                downstream_service,
                path_template,
                default_accept,
                identity_encoding,
            )
        }
        other => other,
    }
}

fn is_idempotent_read(method: &str) -> bool {
    method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD")
}

/// A downstream request, built once from the handler request and sent on either transport.
struct Prepared {
    host: String,
    port: u16,
    method: Method,
    uri: Uri,
    /// Forwarded and added headers, `Host` excluded (each transport sets it).
    headers: HeaderMap,
    /// `Some(empty)` for a bodyless method other than GET/HEAD: sent as `Content-Length: 0`.
    body: Option<Vec<u8>>,
}

fn prepare_downstream(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
    default_accept: &str,
    identity_encoding: bool,
) -> Result<Prepared, ProxyError> {
    let resolved_path = resolve_downstream_target(
        path_template,
        &req.path_params,
        &req.query_params,
        req.raw_query.as_deref(),
    )?;
    let host = downstream_host(downstream_service);
    let port = downstream_http_port();

    // Decision B: validate both stacks; convert to legacy Uri only at this edge
    // (no full request-target embedded in error Display).
    let uri = crate::server::request_target::RequestTarget::try_from_path_query(resolved_path)
        .and_then(|t| t.to_legacy_uri())
        .map_err(|_e| ProxyError::invalid_path(ProxyPathReason::InvalidUri))?;

    let method = Method::from_bytes(req.method.as_str().as_bytes())
        .map_err(|_e| ProxyError::InvalidMethod)?;

    let mut headers = HeaderMap::with_capacity(req.headers.len() + 3);
    for (hk, hv) in &req.headers {
        let name = hk.as_ref();
        if skip_forward_request_header(name)
            || (identity_encoding && name.eq_ignore_ascii_case("accept-encoding"))
        {
            continue;
        }
        if let (Ok(hname), Ok(hval)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(hv.as_str()),
        ) {
            headers.insert(hname, hval);
        }
    }

    if identity_encoding {
        headers.insert(
            http_legacy::header::ACCEPT_ENCODING,
            HeaderValue::from_static("identity"),
        );
    }
    // W3C trace context: this hop is the downstream request's parent (replacing the client's).
    if let Some(traceparent) = may_tracing::traceparent_of_current() {
        if let Ok(value) = HeaderValue::from_str(&traceparent) {
            headers.insert(HeaderName::from_static("traceparent"), value);
        }
    }
    if !headers.contains_key(http_legacy::header::ACCEPT) {
        if let Ok(safe_accept) = HeaderValue::from_str(default_accept) {
            headers.insert(http_legacy::header::ACCEPT, safe_accept);
        }
    }

    let body = match &req.body {
        Some(body_json) => {
            Some(serde_json::to_vec(body_json).map_err(|_e| ProxyError::BodySerialize)?)
        }
        // No body: say so with `Content-Length: 0`. Left alone, the client opens a chunked body
        // for a POST and writes its terminating chunk only after the head; a BRRTRouter
        // downstream answers on the head and may close first, so the terminator's write failed
        // and the BFF answered 502 `request` to a call the downstream had served (pp, 26 Oct
        // 2026: ~40 % of `POST /billing/test-subscriptions/reset`).
        None if method != Method::GET && method != Method::HEAD => Some(Vec::new()),
        None => None,
    };

    Ok(Prepared {
        host,
        port,
        method,
        uri,
        headers,
        body,
    })
}

fn send_downstream_once(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
    default_accept: &str,
    identity_encoding: bool,
) -> Result<(HttpClient, DownstreamResponse), ProxyError> {
    let prepared = prepare_downstream(
        req,
        downstream_service,
        path_template,
        default_accept,
        identity_encoding,
    )?;
    let Prepared {
        host,
        port,
        method,
        uri,
        headers,
        body,
    } = prepared;

    let target = resolve_downstream(&host, port)?;
    let mut client = connect_downstream_with(
        &host,
        port,
        target,
        &system_lookup,
        HttpClient::connect,
        connect_retry_delay(),
    )?;
    client.set_timeout(Some(proxy_timeout()));

    let mut proxy_req = client.new_request(method, uri);
    proxy_req.headers_mut().extend(headers);

    // The client's Host named this service and is dropped by `skip_forward_request_header`.
    // `HttpClient::connect` has no URL to take a Host from and the target is origin-form, so
    // without this the downstream gets no Host at all (strict HTTP/1.1 servers answer 400).
    if let Ok(host_value) = HeaderValue::try_from(crate::http::host_header_value(&host, port, 80)) {
        proxy_req
            .headers_mut()
            .insert(http_legacy::header::HOST, host_value);
    }

    if let Some(body_bytes) = &body {
        proxy_req
            .send(body_bytes)
            .map_err(|e| classify_transport_error(ProxyTransportKind::Request, &e.to_string()))?;
    }

    let rsp = client
        .send_request(proxy_req)
        .map_err(|e| classify_transport_error(ProxyTransportKind::Request, &e.to_string()))?;
    Ok((client, rsp))
}

/// Pool settings for proxied calls, read once from the environment (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProxyPoolSettings {
    enabled: bool,
    max: usize,
    max_per_host: usize,
    idle: Duration,
    lifetime: Duration,
}

impl ProxyPoolSettings {
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let number = |name: &str, default: u64| -> u64 {
            lookup(name)
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(default)
        };
        let enabled = !lookup("BRRTR_PROXY_POOL").is_some_and(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "off" | "0" | "false" | "no"
            )
        });
        let max = number("BRRTR_PROXY_POOL_MAX", 1024) as usize;
        Self {
            enabled,
            max,
            max_per_host: (number("BRRTR_PROXY_POOL_MAX_PER_HOST", 256) as usize).min(max),
            idle: Duration::from_secs(number("BRRTR_PROXY_POOL_IDLE_SECS", 30)),
            lifetime: Duration::from_secs(number("BRRTR_PROXY_POOL_LIFETIME_SECS", 60)),
        }
    }
}

/// Resolves downstream hosts through the proxy's DNS cache ([`resolve_downstream`]).
struct ProxyResolver;

impl Resolver for ProxyResolver {
    fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        resolve_downstream(host, port)
            .map(|resolved| vec![resolved.addr])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, e.to_string()))
    }
}

static PROXY_POOL: OnceLock<Option<Client>> = OnceLock::new();

/// The proxy pool if it has been started (does not start it).
pub(super) fn proxy_pool_if_started() -> Option<&'static Client> {
    PROXY_POOL.get().and_then(Option::as_ref)
}

/// The proxy's connection pool; `None` when it is off or could not be built.
fn proxy_pool() -> Option<&'static Client> {
    PROXY_POOL.get_or_init(|| {
        let settings = ProxyPoolSettings::from_lookup(|name| std::env::var(name).ok());
        if !settings.enabled {
            return None;
        }
        // Downstreams are plain HTTP; the client still wants a TLS configuration.
        let tls = super::fetch::empty_roots_tls_config()?;
        let built = Client::builder()
            .tls_config(tls)
            .resolver(Arc::new(ProxyResolver))
            .max_connections(settings.max)
            .max_connections_per_origin(settings.max_per_host)
            .idle_timeout(settings.idle)
            .max_connection_lifetime(settings.lifetime)
            .redirect_policy(RedirectPolicy::None)
            // A forwarded request carries the client's headers: allow what the server accepts.
            .max_request_headers(256)
            .max_request_header_bytes(64 * 1024)
            .build();
        match built {
            Ok(pool) => Some(pool),
            Err(error) => {
                tracing::error!(error = %error, "proxy connection pool could not be built; proxied calls open a connection each");
                None
            }
        }
    })
    .as_ref()
}

/// Connection reuse statistics of the proxy pool (`None` when pooling is off).
#[must_use]
pub fn proxy_pool_stats() -> Option<may_minihttp::client::ClientStats> {
    proxy_pool().map(Client::stats)
}

/// One pooled attempt. Headers and body are moved into the request, not copied.
fn send_pooled_once(pool: &Client, prepared: Prepared) -> Result<StreamingResponse, ProxyError> {
    let url = format!(
        "http://{}{}",
        crate::http::host_header_value(&prepared.host, prepared.port, 80),
        prepared.uri
    );
    let mut request = pool
        .request(prepared.method, &url)
        .map_err(|_e| ProxyError::invalid_path(ProxyPathReason::InvalidUri))?
        .timeout(proxy_timeout());
    // `HeaderMap::into_iter` names a header once, then yields its further values with `None`.
    let mut last: Option<HeaderName> = None;
    for (name, value) in prepared.headers {
        if let Some(name) = name {
            last = Some(name);
        }
        if let Some(name) = &last {
            request = request.header(name.clone(), value);
        }
    }
    if let Some(body) = prepared.body {
        request = request.body(body);
    }
    request.send_streaming().map_err(pooled_error)
}

fn pooled_error(error: std::io::Error) -> ProxyError {
    let error = ClientError::from(error);
    let message = error.to_string();
    match error.kind() {
        ClientErrorKind::Dns => ProxyError::Dns,
        ClientErrorKind::Timeout => ProxyError::Timeout,
        ClientErrorKind::Connection if message.to_ascii_lowercase().contains("refused") => {
            ProxyError::Connect
        }
        _ => classify_transport_error(ProxyTransportKind::Request, &message),
    }
}

/// Send `req` downstream on a pooled connection and return the response with its body unread.
///
/// Retries mirror the per-call path: a refused connect (a stale cached address, or a pod leaving
/// in a rollout) drops the cached address and tries once more after `PROXY_CONNECT_RETRY_MS`;
/// nothing was sent. A GET/HEAD cut off after sending is sent once more. A timeout is not
/// retried. (The pool itself also replaces an idle connection the downstream closed, and retries
/// a GET/HEAD once when a reused connection fails.)
fn send_downstream_pooled(
    pool: &Client,
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
    default_accept: &str,
) -> Result<StreamingResponse, ProxyError> {
    // Built per attempt (a retry is rare), so the first attempt can move it into the request.
    let attempt = || {
        let prepared = prepare_downstream(
            req,
            downstream_service,
            path_template,
            default_accept,
            false,
        )?;
        send_pooled_once(pool, prepared)
    };
    match attempt() {
        Err(ProxyError::Connect) => {
            forget_downstream_host(&downstream_host(downstream_service));
            if let Some(delay) = connect_retry_delay() {
                // Yields the coroutine; a plain thread sleeps.
                may::coroutine::sleep(delay);
            }
            attempt()
        }
        Err(ProxyError::Request) if is_idempotent_read(req.method.as_str()) => attempt(),
        other => other,
    }
}

/// Proxy an `x-sse` BFF route to a downstream event stream, relaying each event
/// as it arrives.
///
/// Frames pass through byte for byte, so `event:`, `id:` and `retry:` lines and
/// comments reach the client; [`proxy_untyped`] would buffer the whole stream.
/// A downstream that answers with anything but a `2xx` `text/event-stream` (an
/// error, a 401) is answered as [`proxy_untyped`] answers it. The relay ends
/// when either side closes, or after [`PROXY_SSE_IDLE_ENV`] of silence.
#[must_use]
pub fn proxy_sse(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
) -> HandlerResponse {
    let started = std::time::Instant::now();
    let result = proxy_sse_inner(req, downstream_service, path_template);
    // for a stream, the time to the downstream's response headers
    record_call(req, downstream_service, &result, started);
    match result {
        Ok(res) => res,
        Err(e) => proxy_error_response(&e),
    }
}

fn proxy_sse_idle() -> Duration {
    std::env::var(PROXY_SSE_IDLE_ENV)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|secs| Duration::from_secs(secs.max(1)))
        .unwrap_or(DEFAULT_PROXY_SSE_IDLE)
}

fn proxy_sse_inner(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
) -> Result<HandlerResponse, ProxyError> {
    let (mut client, mut rsp) = send_downstream(
        req,
        downstream_service,
        path_template,
        "text/event-stream",
        true,
    )?;
    let status = rsp.status().as_u16();
    let content_type = rsp
        .headers()
        .get(http_legacy::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let is_stream = (200..300).contains(&status)
        && content_type.as_deref().is_some_and(|ct| {
            ct.trim_start()
                .to_ascii_lowercase()
                .starts_with("text/event-stream")
        });

    // The relay reads frames out of the body: it must be plain text. The request
    // asked for identity; a downstream that compresses anyway is refused.
    let encoded = rsp
        .headers()
        .get(http_legacy::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|e| !e.trim().is_empty() && !e.trim().eq_ignore_ascii_case("identity"));
    if is_stream && encoded {
        return Err(ProxyError::Response);
    }

    let mut headers = HeaderVec::new();
    for (name, value) in rsp.headers().iter() {
        let name = name.as_str();
        if skip_forward_response_header(name)
            || (is_stream
                && (name.eq_ignore_ascii_case("content-type")
                    || name.eq_ignore_ascii_case("cache-control")))
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            headers.push((Arc::from(name), v.to_string()));
        }
    }

    if !is_stream {
        let buf = read_bounded_body(&mut rsp, MAX_PROXY_BODY_BYTES, None)?;
        let body = response_body_value(&buf, content_type.as_deref());
        if !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        {
            headers.push((
                Arc::from("content-type"),
                content_type.unwrap_or_else(|| "application/json".to_string()),
            ));
        }
        return Ok(HandlerResponse::new(status, headers, body));
    }

    client.set_timeout(Some(proxy_sse_idle()));
    let (tx, rx) =
        crate::sse::channel_limited(crate::sse::queue_bound_from_env(), MAX_SSE_QUEUED_BYTES);
    // SAFETY: the coroutine owns the connection, the response and the sender,
    // touches no thread-local state, and ends when either side closes.
    let spawned = unsafe {
        may::coroutine::Builder::new()
            .name("brrtr-proxy-sse".to_owned())
            .stack_size(0x10000)
            .spawn(move || relay_sse(client, rsp, &tx))
    };
    if spawned.is_err() {
        return Err(classify_transport_error(
            ProxyTransportKind::Response,
            "could not start the event stream relay",
        ));
    }
    let mut out = HandlerResponse::sse_live(rx);
    out.status = status;
    out.headers.extend(headers);
    Ok(out)
}

/// Read the downstream stream and queue each complete event frame, until the
/// downstream closes or goes quiet, or the client goes away or falls too far
/// behind.
fn relay_sse(client: HttpClient, mut rsp: DownstreamResponse, tx: &crate::sse::SseSender) {
    let _connection = client;
    relay_frames(&mut rsp, tx);
}

fn relay_frames(body: &mut impl Read, tx: &crate::sse::SseSender) {
    let mut chunk = vec![0u8; 8192];
    let mut pending: Vec<u8> = Vec::new();
    // A frame that ended on a CR at the end of what had arrived: if the next byte
    // is LF, the two were one CRLF line end, already relayed.
    let mut skip_lf = false;
    loop {
        let n = match body.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let mut new = &chunk[..n];
        if std::mem::take(&mut skip_lf) && new.first() == Some(&b'\n') {
            new = &new[1..];
        }
        pending.extend_from_slice(new);
        while let Some(end) = sse_frame_end(&pending) {
            let frame: Vec<u8> = pending.drain(..end).collect();
            skip_lf = pending.is_empty() && frame.last() == Some(&b'\r');
            if tx
                .send_frame(String::from_utf8_lossy(&frame).into_owned())
                .is_err()
            {
                return;
            }
        }
        if pending.len() > MAX_SSE_FRAME_BYTES {
            return;
        }
    }
}

/// End (exclusive) of the first complete event frame in `buf`: just past the
/// first empty line. Event streams may end lines with CRLF, LF or CR. A CR at the
/// very end of `buf` ends the frame when it closes the empty line (CR or CRLF,
/// the frame is complete either way); otherwise the next byte decides.
fn sse_frame_end(buf: &[u8]) -> Option<usize> {
    let mut after_line_end = false;
    let mut i = 0;
    while i < buf.len() {
        let eol = match buf[i] {
            b'\n' => 1,
            b'\r' if buf.get(i + 1) == Some(&b'\n') => 2,
            b'\r' if i + 1 == buf.len() && !after_line_end => return None,
            b'\r' => 1,
            _ => {
                after_line_end = false;
                i += 1;
                continue;
            }
        };
        if after_line_end {
            return Some(i + eol);
        }
        after_line_end = true;
        i += eol;
    }
    None
}

/// Proxy an untyped BFF route to a downstream Kubernetes Service.
///
/// `downstream_service` is the OpenAPI `x-service` value (e.g. `fleet`).
/// `path_template` is `x-brrtrouter-downstream-path` with `{param}` placeholders.
#[must_use]
pub fn proxy_untyped(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
) -> HandlerResponse {
    let started = std::time::Instant::now();
    let result = proxy_untyped_inner(
        req,
        downstream_service,
        path_template,
        passthrough_enabled(),
    );
    record_call(req, downstream_service, &result, started);
    match result {
        Ok(res) => res,
        Err(e) => proxy_error_response(&e),
    }
}

/// One downstream call into `http_client_request_duration_seconds` (see [`super::client_metrics`]). A request that
/// never left (an invalid path, a target too long) is not a call to the downstream and is not recorded.
fn record_call(
    req: &HandlerRequest,
    downstream_service: &str,
    result: &Result<HandlerResponse, ProxyError>,
    started: std::time::Instant,
) {
    let (status, error) = match result {
        Ok(res) => (Some(res.status), None),
        Err(
            ProxyError::InvalidMethod
            | ProxyError::InvalidPath { .. }
            | ProxyError::RequestTargetTooLong { .. }
            | ProxyError::BodySerialize,
        ) => return,
        Err(e) => (None, Some(proxy_error_reason_code(e))),
    };
    super::client_metrics::record(
        downstream_service,
        req.method.as_str(),
        status,
        error,
        started.elapsed(),
    );
}

fn proxy_untyped_inner(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
    passthrough_on: bool,
) -> Result<HandlerResponse, ProxyError> {
    if let Some(pool) = proxy_pool() {
        let mut rsp = send_downstream_pooled(
            pool,
            req,
            downstream_service,
            path_template,
            "application/json",
        )?;
        let status = rsp.status().as_u16();
        let headers = std::mem::take(rsp.headers_mut());
        return untyped_response(req, status, &headers, &mut rsp, passthrough_on);
    }
    let (_connection, mut rsp) = send_downstream(
        req,
        downstream_service,
        path_template,
        "application/json",
        false,
    )?;
    let status = rsp.status().as_u16();
    let headers = std::mem::take(rsp.headers_mut());
    untyped_response(req, status, &headers, &mut rsp, passthrough_on)
}

/// Read the downstream body and build the handler response (either transport).
fn untyped_response(
    req: &HandlerRequest,
    status: u16,
    headers: &HeaderMap,
    body: &mut impl Read,
    passthrough_on: bool,
) -> Result<HandlerResponse, ProxyError> {
    // A HEAD answer advertises the GET body's length but carries none.
    let size_hint = headers
        .get(http_legacy::header::CONTENT_LENGTH)
        .filter(|_| req.method != http::Method::HEAD)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok());
    let buf = read_bounded_body(body, MAX_PROXY_BODY_BYTES, size_hint)?;
    let content_type = headers
        .get(http_legacy::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    // With pass-through on (ADR 006 §2), a 2xx JSON body goes back as the downstream wrote it: no
    // parse into a Value here and no serialisation again on the way out; response validation then
    // parses it only when the mode checks it. Errors keep the parsed path: their reason is read for
    // logs and metrics. Off (the default), the body is parsed as it always was.
    let passthrough = if passthrough_on {
        passthrough_body(buf, status, content_type)
    } else {
        Err(buf)
    };
    let (body_json, raw_json) = match passthrough {
        Ok(text) => (Value::String(text), true),
        Err(buf) => (response_body_value(&buf, content_type), false),
    };

    let mut out_headers = HeaderVec::new();
    for (name, value) in headers.iter() {
        if skip_forward_response_header(name.as_str()) {
            continue;
        }
        if let Ok(s) = value.to_str() {
            out_headers.push((Arc::from(name.as_str()), s.to_string()));
        }
    }
    if !out_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        out_headers.push((
            Arc::from("content-type"),
            content_type.unwrap_or("application/json").to_string(),
        ));
    }
    if raw_json {
        out_headers.push((
            Arc::from(crate::server::response::RAW_BODY_ENCODING_HEADER),
            crate::server::response::RAW_BODY_ENCODING_JSON.to_string(),
        ));
    }

    Ok(HandlerResponse::new(status, out_headers, body_json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::HandlerRequest;
    use crate::ids::RequestId;
    use crate::router::ParamVec;
    use http::Method;
    use may::sync::mpsc;
    use std::sync::{Arc, Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Serialize against length-limit tests that mutate
    /// `BRRTROUTER_MAX_REQUEST_TARGET_OCTETS` + the process-global cache.
    /// Without this, parallel `--lib` runs can make
    /// `resolve_downstream_target(...).unwrap()` fail with `RequestTargetTooLong`.
    fn lock_default_request_target_limit() -> std::sync::MutexGuard<'static, ()> {
        let guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        guard
    }

    fn empty_request(method: Method) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel();
        HandlerRequest {
            request_id: RequestId::new(),
            method,
            path: "/api/v1/fleet/vehicles".to_string(),
            handler_name: "proxy_test".to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            raw_query: None,
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
            span: tracing::Span::none(),
        }
    }

    fn param_vec(pairs: &[(&str, &str)]) -> ParamVec {
        let mut params = ParamVec::new();
        for (k, v) in pairs {
            params.push((Arc::from(*k), (*v).to_string()));
        }
        params
    }

    fn assert_parseable_uri(path: &str) {
        path.parse::<Uri>()
            .unwrap_or_else(|e| panic!("URI rejected {path:?}: {e}"));
    }

    /// Pre-fix behaviour: decoded values concatenated raw → InvalidUri.
    fn legacy_unencoded_query(path_template: &str, query: &[(&str, &str)]) -> String {
        let mut path = path_template.to_string();
        if !query.is_empty() {
            path.push('?');
            for (i, (k, v)) in query.iter().enumerate() {
                if i > 0 {
                    path.push('&');
                }
                path.push_str(k);
                path.push('=');
                path.push_str(v);
            }
        }
        path
    }

    #[test]
    fn resolve_path_template_substitutes_params_and_query() {
        let path = resolve_path_template(
            "/api/v1/fleet/vehicles/{id}",
            &param_vec(&[("id", "abc")]),
            &param_vec(&[("limit", "10")]),
        );
        assert_eq!(path, "/api/v1/fleet/vehicles/abc?limit=10");
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_percent_encodes_query_space() {
        // Incident: country=South Africa (decoded) → InvalidUri → BFF 502.
        let path = resolve_path_template(
            "/api/v1/locations/provinces",
            &ParamVec::new(),
            &param_vec(&[("country", "South Africa")]),
        );
        assert_eq!(path, "/api/v1/locations/provinces?country=South%20Africa");
        assert_parseable_uri(&path);
        let legacy = legacy_unencoded_query(
            "/api/v1/locations/provinces",
            &[("country", "South Africa")],
        );
        assert!(
            legacy.parse::<Uri>().is_err(),
            "regression guard: unencoded space must fail Uri parse"
        );
    }

    #[test]
    fn resolve_path_template_ascii_safe_query_unchanged() {
        let path = resolve_path_template(
            "/api/v1/locations/provinces",
            &ParamVec::new(),
            &param_vec(&[("country", "ZA")]),
        );
        assert_eq!(path, "/api/v1/locations/provinces?country=ZA");
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_encodes_accents_and_diacritics() {
        let cases = [
            ("Côte d'Ivoire", "C%C3%B4te%20d%27Ivoire"),
            ("São Paulo", "S%C3%A3o%20Paulo"),
            ("Québec", "Qu%C3%A9bec"),
            ("Zürich", "Z%C3%BCrich"),
            ("España", "Espa%C3%B1a"),
            ("Österreich", "%C3%96sterreich"),
        ];
        for (raw, encoded) in cases {
            let path = resolve_path_template(
                "/api/v1/locations/provinces",
                &ParamVec::new(),
                &param_vec(&[("country", raw)]),
            );
            assert_eq!(
                path,
                format!("/api/v1/locations/provinces?country={encoded}"),
                "raw={raw:?}"
            );
            assert_parseable_uri(&path);
            let decoded = urlencoding::decode(
                path.strip_prefix("/api/v1/locations/provinces?country=")
                    .expect("prefix"),
            )
            .expect("decode");
            assert_eq!(decoded.as_ref(), raw);
        }
    }

    #[test]
    fn resolve_path_template_encodes_query_delimiter_chars() {
        // Delimiters in *values* must not split/corrupt the query string.
        let path = resolve_path_template(
            "/search",
            &ParamVec::new(),
            &param_vec(&[("q", "a&b=c?d#e"), ("tag", "x y")]),
        );
        assert_eq!(path, "/search?q=a%26b%3Dc%3Fd%23e&tag=x%20y");
        assert_parseable_uri(&path);
        let uri: Uri = path.parse().unwrap();
        let qs = uri.query().unwrap_or("");
        assert!(qs.contains("a%26b"), "ampersand must stay encoded in value");
        assert!(!qs.contains("a&b="), "raw & must not introduce a new param");
    }

    #[test]
    fn resolve_path_template_encodes_plus_percent_and_unicode() {
        let path = resolve_path_template(
            "/api/v1/locations/cities",
            &ParamVec::new(),
            &param_vec(&[
                ("name", "C++"),
                ("note", "100%"),
                ("city", "東京"),
                ("emoji", "🚛"),
            ]),
        );
        assert_parseable_uri(&path);
        assert!(path.contains("name=C%2B%2B"));
        assert!(path.contains("note=100%25"));
        assert!(path.contains("city=%E6%9D%B1%E4%BA%AC"));
        assert!(path.contains("emoji=%F0%9F%9A%9B"));
    }

    #[test]
    fn resolve_path_template_empty_query_value_and_multiple_params() {
        let path = resolve_path_template(
            "/api/v1/locations/countries",
            &ParamVec::new(),
            &param_vec(&[
                ("origin", "ZA"),
                ("home_country", ""),
                ("q", "KwaZulu-Natal"),
            ]),
        );
        assert_eq!(
            path,
            "/api/v1/locations/countries?origin=ZA&home_country=&q=KwaZulu-Natal"
        );
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_encodes_path_param_space_and_accents() {
        let path = resolve_path_template(
            "/api/v1/regions/{name}/summary",
            &param_vec(&[("name", "Western Cape")]),
            &ParamVec::new(),
        );
        assert_eq!(path, "/api/v1/regions/Western%20Cape/summary");
        assert_parseable_uri(&path);

        let path = resolve_path_template(
            "/api/v1/regions/{name}/summary",
            &param_vec(&[("name", "Provence-Alpes-Côte d'Azur")]),
            &ParamVec::new(),
        );
        assert!(path.contains("C%C3%B4te"));
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_encodes_slash_in_path_param() {
        // Path params are single OpenAPI segments; `/` must not create extra segments.
        let path = resolve_path_template(
            "/api/v1/docs/{id}",
            &param_vec(&[("id", "a/b")]),
            &ParamVec::new(),
        );
        assert_eq!(path, "/api/v1/docs/a%2Fb");
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_encodes_query_keys() {
        let path = resolve_path_template(
            "/q",
            &ParamVec::new(),
            &param_vec(&[("filter name", "yes")]),
        );
        assert_eq!(path, "/q?filter%20name=yes");
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_encodes_control_and_whitespace() {
        let path = resolve_path_template("/q", &ParamVec::new(), &param_vec(&[("q", "a\tb\nc")]));
        assert_eq!(path, "/q?q=a%09b%0Ac");
        assert_parseable_uri(&path);
    }

    #[test]
    fn resolve_path_template_negative_unencoded_space_and_controls_fail_uri_parse() {
        // Spaces / ASCII controls in the query break http::Uri (incident class).
        for v in ["South Africa", "line\nbreak", "tab\there"] {
            let legacy = legacy_unencoded_query("/p", &[("k", v)]);
            assert!(
                legacy.parse::<Uri>().is_err(),
                "expected InvalidUri for unencoded {v:?} in {legacy:?}"
            );
            let fixed = resolve_path_template("/p", &ParamVec::new(), &param_vec(&[("k", v)]));
            assert_parseable_uri(&fixed);
        }
    }

    #[test]
    fn resolve_path_template_negative_unencoded_hash_truncates_query() {
        // `#` is accepted by Uri parse but steals the remainder as a fragment.
        let legacy = legacy_unencoded_query("/p", &[("k", "x#frag")]);
        assert!(legacy.contains("#frag"));
        let legacy_uri: Uri = legacy.parse().expect("hash form still parses");
        assert_eq!(legacy_uri.query(), Some("k=x"));
        let fixed = resolve_path_template("/p", &ParamVec::new(), &param_vec(&[("k", "x#frag")]));
        assert_eq!(fixed, "/p?k=x%23frag");
        assert!(!fixed.contains('#'));
        let fixed_uri: Uri = fixed.parse().unwrap();
        assert_eq!(fixed_uri.query(), Some("k=x%23frag"));
    }

    #[test]
    fn resolve_path_template_negative_unencoded_ampersand_corrupts_query() {
        // `&` / `=` often still parse as a URI but invent extra query params.
        let legacy = legacy_unencoded_query("/p", &[("k", "a&b=c")]);
        let legacy_uri: Uri = legacy.parse().expect("ampersand still parses as Uri");
        assert_eq!(legacy_uri.query(), Some("k=a&b=c"));
        let fixed = resolve_path_template("/p", &ParamVec::new(), &param_vec(&[("k", "a&b=c")]));
        assert_eq!(fixed, "/p?k=a%26b%3Dc");
        let fixed_uri: Uri = fixed.parse().unwrap();
        assert_eq!(fixed_uri.query(), Some("k=a%26b%3Dc"));
    }

    #[test]
    fn resolve_path_template_encodes_non_ascii_even_if_uri_would_accept_raw() {
        // Accents must be encoded for interoperable downstream HTTP clients,
        // even when a particular Uri parser is lenient about raw UTF-8.
        let raw = "Côte d'Ivoire";
        let path = resolve_path_template("/p", &ParamVec::new(), &param_vec(&[("country", raw)]));
        assert!(
            !path.contains("ô") && !path.contains('\''),
            "expected percent-encoding, got {path}"
        );
        assert!(path.contains("C%C3%B4te"));
        assert_parseable_uri(&path);
        let encoded = path.rsplit('=').next().unwrap();
        assert_eq!(urlencoding::decode(encoded).unwrap(), raw);
    }

    #[test]
    fn proxy_error_invalid_path_display() {
        let err = ProxyError::invalid_path(ProxyPathReason::InvalidUri);
        assert_eq!(err.to_string(), "invalid path: invalid uri character");
    }

    #[test]
    fn downstream_host_uses_pod_namespace_when_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("POD_NAMESPACE", "logistics");
        assert_eq!(
            downstream_host("fleet"),
            "fleet.logistics.svc.cluster.local"
        );
        std::env::remove_var("POD_NAMESPACE");
    }

    #[test]
    fn downstream_host_short_name_without_namespace_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        assert_eq!(downstream_host("fleet"), "fleet");
    }

    #[test]
    fn downstream_http_port_defaults_to_8080() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("SERVICE_HTTP_PORT");
        assert_eq!(downstream_http_port(), 8080);
    }

    #[test]
    fn downstream_http_port_reads_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("SERVICE_HTTP_PORT", "9090");
        assert_eq!(downstream_http_port(), 9090);
        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    #[test]
    fn lookup_name_makes_service_fqdns_absolute() {
        assert_eq!(
            lookup_name("market.pw.svc.cluster.local"),
            "market.pw.svc.cluster.local."
        );
        assert_eq!(lookup_name("market"), "market");
        assert_eq!(lookup_name("localhost"), "localhost");
        assert_eq!(lookup_name("api.example.com"), "api.example.com");
    }

    #[test]
    fn resolve_downstream_caches_and_forgets() {
        let host = "localhost";
        let a = resolve_downstream(host, 18_081).expect("localhost resolves");
        assert_eq!(a.addr.port(), 18_081);
        assert!(!a.cached);
        // cached for the same port
        let again = resolve_downstream(host, 18_081).expect("cached");
        assert_eq!(
            again,
            Resolved {
                addr: a.addr,
                cached: true
            }
        );
        // another port is a miss, re-resolved and re-cached
        let b = resolve_downstream(host, 18_082).expect("localhost resolves");
        assert_eq!(b.addr.port(), 18_082);
        assert!(!b.cached);
        // forgetting only drops the address it was asked about
        forget_downstream(host, a.addr);
        assert!(dns_cache().read().unwrap().get(host).is_some());
        forget_downstream(host, b.addr);
        assert!(dns_cache().read().unwrap().get(host).is_none());
    }

    /// A Service recreated under N in-flight requests that all read the stale cached address:
    /// every one of them re-resolves and connects (none fails), the system resolver runs once,
    /// and the fresh address stays cached.
    fn stale_address_scenario(host: &'static str, concurrent: bool) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        const N: usize = 8;
        let stale: SocketAddr = "10.0.0.1:8080".parse().unwrap();
        let fresh: SocketAddr = "10.0.0.2:8080".parse().unwrap();
        dns_cache()
            .write()
            .unwrap()
            .insert(host.to_string(), (8080, stale, Instant::now()));
        let lookups = Arc::new(AtomicUsize::new(0));
        let lookup = {
            let lookups = Arc::clone(&lookups);
            move |_: &str, _: u16| {
                lookups.fetch_add(1, Ordering::SeqCst);
                Ok(fresh)
            }
        };
        let connect = move |addr: SocketAddr| {
            if addr == fresh {
                Ok(addr)
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused))
            }
        };
        // All N read the stale address before any of them tries to connect.
        let resolved: Vec<Resolved> = (0..N)
            .map(|_| resolve_downstream_with(host, 8080, &lookup).unwrap())
            .collect();
        assert!(resolved.iter().all(|r| *r
            == Resolved {
                addr: stale,
                cached: true
            }));
        let results: Vec<Result<SocketAddr, ProxyError>> = if concurrent {
            let barrier = Arc::new(std::sync::Barrier::new(N));
            let handles: Vec<_> = resolved
                .into_iter()
                .map(|r| {
                    let barrier = Arc::clone(&barrier);
                    let lookup = lookup.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        connect_downstream_with(host, 8080, r, &lookup, connect, TEST_RETRY)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        } else {
            resolved
                .into_iter()
                .map(|r| connect_downstream_with(host, 8080, r, &lookup, connect, TEST_RETRY))
                .collect()
        };
        assert!(
            results.iter().all(|r| matches!(r, Ok(a) if *a == fresh)),
            "every request that held the stale address retried: {results:?}"
        );
        let n = lookups.load(Ordering::SeqCst);
        assert!(n >= 1 && n <= N, "lookups {n}");
        if !concurrent {
            assert_eq!(n, 1, "later requests reuse the re-resolved address");
        }
        assert_eq!(
            dns_cache().read().unwrap().get(host).map(|e| e.1),
            Some(fresh)
        );
    }

    #[test]
    fn stale_cached_address_every_request_retries() {
        stale_address_scenario("stale-svc-sequential.test", false);
    }

    #[test]
    fn stale_cached_address_every_concurrent_request_retries() {
        stale_address_scenario("stale-svc-concurrent.test", true);
    }

    /// A short retry pause passed explicitly, so the tests do not depend on
    /// `PROXY_CONNECT_RETRY_MS` in the environment.
    const TEST_RETRY: Option<Duration> = Some(Duration::from_millis(1));

    #[test]
    fn uncached_address_failure_is_retried_once_without_lookup() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let lookups = AtomicUsize::new(0);
        let lookup = |_: &str, _: u16| {
            lookups.fetch_add(1, Ordering::SeqCst);
            Ok("10.0.0.3:8080".parse().unwrap())
        };
        let r = Resolved {
            addr: "10.0.0.3:8080".parse().unwrap(),
            cached: false,
        };
        let connects = AtomicUsize::new(0);
        let res = connect_downstream_with(
            "fresh-svc.test",
            8080,
            r,
            &lookup,
            |_| {
                connects.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(std::io::Error::from(std::io::ErrorKind::ConnectionRefused))
            },
            TEST_RETRY,
        );
        assert!(res.is_err());
        assert_eq!(lookups.load(Ordering::SeqCst), 0);
        assert_eq!(
            connects.load(Ordering::SeqCst),
            2,
            "one retry after the pause"
        );
    }

    #[test]
    fn uncached_address_timeout_is_not_retried() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let lookup = |_: &str, _: u16| Ok("10.0.0.5:8080".parse().unwrap());
        let r = Resolved {
            addr: "10.0.0.5:8080".parse().unwrap(),
            cached: false,
        };
        let connects = AtomicUsize::new(0);
        let res = connect_downstream_with(
            "slow-svc.test",
            8080,
            r,
            &lookup,
            |_| {
                connects.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(std::io::Error::from(std::io::ErrorKind::TimedOut))
            },
            TEST_RETRY,
        );
        assert!(matches!(res, Err(ProxyError::Timeout)));
        assert_eq!(connects.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn uncached_address_second_attempt_succeeds() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let lookup = |_: &str, _: u16| Ok("10.0.0.4:8080".parse().unwrap());
        let r = Resolved {
            addr: "10.0.0.4:8080".parse().unwrap(),
            cached: false,
        };
        let connects = AtomicUsize::new(0);
        let res = connect_downstream_with(
            "flappy-svc.test",
            8080,
            r,
            &lookup,
            |_| {
                if connects.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused))
                } else {
                    Ok(7u8)
                }
            },
            TEST_RETRY,
        );
        assert_eq!(res.unwrap(), 7);
    }

    #[test]
    fn proxy_pool_settings_default_and_from_environment() {
        let defaults = ProxyPoolSettings::from_lookup(|_| None);
        assert_eq!(
            defaults,
            ProxyPoolSettings {
                enabled: true,
                max: 1024,
                max_per_host: 256,
                idle: Duration::from_secs(30),
                lifetime: Duration::from_secs(60),
            }
        );
        let custom = ProxyPoolSettings::from_lookup(|name| {
            match name {
                "BRRTR_PROXY_POOL_MAX" => Some("16"),
                "BRRTR_PROXY_POOL_MAX_PER_HOST" => Some("32"), // capped at max
                "BRRTR_PROXY_POOL_LIFETIME_SECS" => Some("5"),
                "BRRTR_PROXY_POOL_IDLE_SECS" => Some("x"), // invalid: default
                _ => None,
            }
            .map(str::to_string)
        });
        assert_eq!(custom.max_per_host, 16);
        assert_eq!(custom.lifetime, Duration::from_secs(5));
        assert_eq!(custom.idle, Duration::from_secs(30));
        assert!(
            !ProxyPoolSettings::from_lookup(|n| (n == "BRRTR_PROXY_POOL").then(|| "off".into()))
                .enabled
        );
    }

    #[test]
    fn client_pool_key_formats_host_port() {
        assert_eq!(
            client_pool_key("fleet.logistics.svc.cluster.local", 8080),
            "fleet.logistics.svc.cluster.local:8080"
        );
    }

    #[test]
    fn skip_forward_request_header_blocks_hop_by_hop() {
        assert!(skip_forward_request_header("Host"));
        assert!(skip_forward_request_header("connection"));
        assert!(!skip_forward_request_header("Authorization"));
    }

    #[test]
    fn skip_forward_response_header_blocks_hop_by_hop() {
        assert!(skip_forward_response_header("Transfer-Encoding"));
        assert!(skip_forward_response_header("Content-Length"));
        assert!(!skip_forward_response_header("Content-Type"));
        // A downstream cannot set BRRTRouter's internal body marker.
        assert!(skip_forward_response_header("X-BRRTRouter-Raw-Encoding"));
    }

    #[test]
    fn read_bounded_body_caps_the_size_hint_reservation() {
        let body = vec![b'x'; 10];
        let buf =
            read_bounded_body(&mut &body[..], MAX_PROXY_BODY_BYTES, Some(usize::MAX)).unwrap();
        assert_eq!(buf, body);
        assert!(
            buf.capacity() <= MAX_PROXY_BODY_PREALLOC + 1,
            "{}",
            buf.capacity()
        );
        let big = vec![b'y'; MAX_PROXY_BODY_PREALLOC + 10];
        let buf = read_bounded_body(&mut &big[..], MAX_PROXY_BODY_BYTES, Some(big.len())).unwrap();
        assert_eq!(
            buf.len(),
            big.len(),
            "a body past the reservation still reads whole"
        );
        assert!(matches!(
            read_bounded_body(&mut &body[..], 5, Some(10)),
            Err(ProxyError::BodyTooLarge)
        ));
    }

    #[test]
    fn response_body_value_parses_json() {
        let v = response_body_value(br#"{"ok":true}"#, Some("application/json"));
        assert_eq!(v, serde_json::json!({"ok": true}));
    }

    #[test]
    fn response_body_value_falls_back_to_string() {
        let v = response_body_value(b"plain text", Some("text/plain"));
        assert_eq!(v, Value::String("plain text".to_string()));
    }

    #[test]
    fn proxy_untyped_returns_502_on_dns_failure() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("POD_NAMESPACE", "logistics");
        std::env::set_var("SERVICE_HTTP_PORT", "8080");
        let req = empty_request(Method::GET);
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/health");
        assert_eq!(res.status, 502);
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    // --- Story 10.5 query passthrough ---

    #[test]
    fn resolve_downstream_positive_p1_passthrough_plus_and_pct2b() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("q", "a+b c")]);
        let raw = "q=a%2Bb+c";
        assert!(query_params_match_raw(raw, &q));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=a%2Bb+c");
    }

    #[test]
    fn resolve_downstream_positive_p2_passthrough_preserves_pct20() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("q", "South Africa")]);
        let raw = "q=South%20Africa";
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=South%20Africa");
        assert!(!t.contains('+'));
    }

    #[test]
    fn resolve_downstream_positive_p3_path_param_rebuild_path() {
        let _lock = lock_default_request_target_limit();
        let path = param_vec(&[("id", "a b")]);
        let t = resolve_downstream_target("/items/{id}", &path, &ParamVec::new(), None).unwrap();
        assert_eq!(t, "/items/a%20b");
        assert_parseable_uri(&t);
    }

    #[test]
    fn resolve_downstream_positive_p4_path_sub_query_passthrough() {
        let _lock = lock_default_request_target_limit();
        let path = param_vec(&[("id", "x")]);
        let q = param_vec(&[("q", "a+b c")]);
        let raw = "q=a%2Bb+c";
        let t = resolve_downstream_target("/items/{id}", &path, &q, Some(raw)).unwrap();
        assert_eq!(t, "/items/x?q=a%2Bb+c");
    }

    #[test]
    fn resolve_downstream_positive_p5_multi_param_passthrough() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("a", "1"), ("b", "2")]);
        let raw = "a=1&b=2";
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?a=1&b=2");
    }

    #[test]
    fn resolve_downstream_positive_p6_empty_query_no_spurious_qmark() {
        let _lock = lock_default_request_target_limit();
        let t =
            resolve_downstream_target("/p", &ParamVec::new(), &ParamVec::new(), Some("")).unwrap();
        assert_eq!(t, "/p");
        let t2 = resolve_downstream_target("/p", &ParamVec::new(), &ParamVec::new(), None).unwrap();
        assert_eq!(t2, "/p");
    }

    #[test]
    fn resolve_downstream_positive_p7_rebuild_space_to_pct20() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("k", "South Africa")]);
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, None).unwrap();
        assert_eq!(t, "/p?k=South%20Africa");
        assert_parseable_uri(&t);
    }

    #[test]
    fn resolve_downstream_positive_p8_unmutated_selects_passthrough() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("q", "x")]);
        let raw = "q=x";
        assert!(query_params_match_raw(raw, &q));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=x");
    }

    #[test]
    fn resolve_downstream_negative_n1_raw_space_rejected() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("q", "a b")]);
        // Crafted match via parse of space-containing raw is unusual; force match by using
        // decoded equivalent but unsafe wire octets.
        let err = resolve_downstream_target("/p", &ParamVec::new(), &q, Some("q=a b")).unwrap_err();
        assert_eq!(
            err,
            ProxyError::invalid_path(ProxyPathReason::UnsafeRawQuery)
        );
    }

    #[test]
    fn resolve_downstream_negative_n2_ctl_and_hash_rejected() {
        let _lock = lock_default_request_target_limit();
        // Unsafe hash with matching parse: form_urlencoded treats `#` as part of value.
        let q_hash = param_vec(&[("q", "a#frag")]);
        let err = resolve_downstream_target("/p", &ParamVec::new(), &q_hash, Some("q=a#frag"))
            .unwrap_err();
        assert_eq!(
            err,
            ProxyError::invalid_path(ProxyPathReason::UnsafeRawQuery)
        );
        assert!(!raw_query_is_wire_safe("q=a\tb"));
    }

    #[test]
    fn resolve_downstream_negative_n3_mutation_forces_rebuild() {
        let _lock = lock_default_request_target_limit();
        let mutated = param_vec(&[("q", "new")]);
        let raw = "q=old";
        assert!(!query_params_match_raw(raw, &mutated));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &mutated, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=new");
        assert!(!t.contains("old"));
    }

    #[test]
    fn resolve_downstream_negative_n4_missing_path_param() {
        let _lock = lock_default_request_target_limit();
        let err =
            resolve_downstream_target("/items/{id}", &ParamVec::new(), &ParamVec::new(), None)
                .unwrap_err();
        assert_eq!(
            err,
            ProxyError::invalid_path(ProxyPathReason::UnresolvedPathParam)
        );
    }

    #[test]
    fn resolve_downstream_negative_n5_no_double_encode_on_passthrough() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("q", "%20")]); // decoded value is literal %20
        let raw = "q=%2520"; // wire was double-encoded once already as inbound
                             // If params match raw parse of %2520 → value "%20"
        assert!(query_params_match_raw(raw, &q));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=%2520", "passthrough must not re-encode");
    }

    #[test]
    fn resolve_downstream_negative_n6_malformed_raw_space() {
        assert!(!raw_query_is_wire_safe("q=a b"));
    }

    #[test]
    fn resolve_downstream_negative_n8_question_in_resolved_path() {
        let _lock = lock_default_request_target_limit();
        // encode_path_segment encodes `?` → should not hit this; force via template without `{`
        // that already contains `?` (misconfig).
        let err = resolve_downstream_target("/p?evil", &ParamVec::new(), &ParamVec::new(), None)
            .unwrap_err();
        assert_eq!(
            err,
            ProxyError::invalid_path(ProxyPathReason::PathContainsQuestion)
        );
    }

    // --- Story 10.6 length / 414 ---

    #[test]
    fn resolve_downstream_length_positive_p1_under_limit() {
        let _lock = ENV_LOCK.lock().unwrap();
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        std::env::set_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS", "100");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        let t = resolve_downstream_target("/p", &ParamVec::new(), &ParamVec::new(), None).unwrap();
        assert_eq!(t, "/p");
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
    }

    #[test]
    fn resolve_downstream_length_positive_p2_at_limit_minus_one() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS", "10");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        // "/p?q=xxxx" = 9 chars
        let q = param_vec(&[("q", "xxxx")]);
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, None).unwrap();
        assert_eq!(t.len(), 9);
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
    }

    #[test]
    fn resolve_downstream_length_positive_p3_exactly_at_limit() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS", "9");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        let q = param_vec(&[("q", "xxxx")]);
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, None).unwrap();
        assert_eq!(t.len(), 9);
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
    }

    #[test]
    fn resolve_downstream_length_positive_p6_default_ge_8192() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        assert!(crate::server::request_target::DEFAULT_MAX_REQUEST_TARGET_OCTETS >= 8192);
        assert_eq!(
            crate::server::request_target::max_request_target_octets(),
            8192
        );
    }

    #[test]
    fn resolve_downstream_length_negative_n2_outbound_over_limit() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS", "8");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        let q = param_vec(&[("q", "xxxx")]); // "/p?q=xxxx" = 9
        let err = resolve_downstream_target("/p", &ParamVec::new(), &q, None).unwrap_err();
        assert!(matches!(err, ProxyError::RequestTargetTooLong { .. }));
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
    }

    #[test]
    fn proxy_untyped_maps_request_target_too_long_to_414() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS", "8");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
        std::env::set_var("POD_NAMESPACE", "logistics");
        let mut req = empty_request(Method::GET);
        req.query_params = param_vec(&[("q", "xxxx")]);
        // DNS would fail anyway, but length check runs first.
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/p");
        assert_eq!(res.status, 414);
        assert_eq!(res.body["reason"], "request_target_too_long");
        std::env::remove_var("BRRTROUTER_MAX_REQUEST_TARGET_OCTETS");
        std::env::remove_var("POD_NAMESPACE");
        crate::server::request_target::reset_max_request_target_cache_for_tests();
    }

    // --- Story 10.7 error taxonomy ---

    fn taxonomy_cases() -> Vec<(ProxyError, u16, &'static str)> {
        vec![
            (ProxyError::InvalidMethod, 400, "invalid_method"),
            (
                ProxyError::invalid_path(ProxyPathReason::InvalidUri),
                400,
                "invalid_uri",
            ),
            (
                ProxyError::invalid_path(ProxyPathReason::UnresolvedPathParam),
                400,
                "unresolved_path_param",
            ),
            (
                ProxyError::invalid_path(ProxyPathReason::PathContainsQuestion),
                400,
                "path_contains_question",
            ),
            (
                ProxyError::invalid_path(ProxyPathReason::UnsafeRawQuery),
                400,
                "unsafe_raw_query",
            ),
            (
                ProxyError::invalid_path(ProxyPathReason::UnsupportedQueryStyle),
                400,
                "unsupported_query_style",
            ),
            (
                ProxyError::RequestTargetTooLong { len: 9, max: 8 },
                414,
                "request_target_too_long",
            ),
            (ProxyError::Dns, 502, "dns"),
            (ProxyError::Connect, 502, "connect"),
            (ProxyError::Request, 502, "request"),
            (ProxyError::Response, 502, "response"),
            (ProxyError::BodyTooLarge, 502, "body_too_large"),
            (ProxyError::Timeout, 504, "timeout"),
            (ProxyError::BodySerialize, 500, "body_serialize"),
        ]
    }

    #[test]
    fn proxy_error_taxonomy_positive_p1_dns_still_502() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("POD_NAMESPACE", "logistics");
        std::env::set_var("SERVICE_HTTP_PORT", "8080");
        let req = empty_request(Method::GET);
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/health");
        assert_eq!(res.status, 502);
        assert_eq!(res.body["reason"], "dns");
        assert_eq!(res.body["error"], "Bad Gateway");
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    #[test]
    fn proxy_error_taxonomy_positive_p2_timeout_maps_to_504() {
        let err = classify_transport_error(ProxyTransportKind::Connect, "connection timed out");
        assert_eq!(err, ProxyError::Timeout);
        assert_eq!(proxy_error_http_status(&err), 504);
        let res = proxy_error_response(&err);
        assert_eq!(res.status, 504);
        assert_eq!(res.body["error"], "Gateway Timeout");
        assert_eq!(res.body["reason"], "timeout");
    }

    #[test]
    fn proxy_error_taxonomy_positive_p3_upstream_status_passthrough_documented() {
        // Upstream HTTP status is returned from Ok(HandlerResponse), not ProxyError.
        // Locked: only Err(*) goes through taxonomy; Ok keeps peer status.
        let upstream = HandlerResponse::error(503, "upstream unavailable");
        assert_eq!(upstream.status, 503);
        assert_ne!(proxy_error_http_status(&ProxyError::Response), 503);
    }

    #[test]
    fn proxy_error_taxonomy_positive_p4_valid_rebuild_no_composition_error() {
        let _lock = lock_default_request_target_limit();
        let q = param_vec(&[("country", "South Africa")]);
        let t = resolve_downstream_target("/provinces", &ParamVec::new(), &q, None).unwrap();
        assert_eq!(t, "/provinces?country=South%20Africa");
        assert_parseable_uri(&t);
    }

    #[test]
    fn proxy_error_taxonomy_positive_p5_overlong_is_414_not_502() {
        let err = ProxyError::RequestTargetTooLong {
            len: 9000,
            max: 8192,
        };
        assert_eq!(proxy_error_http_status(&err), 414);
        assert_ne!(proxy_error_http_status(&err), 502);
    }

    #[test]
    fn proxy_error_taxonomy_positive_p6_stable_reason_labels() {
        let mut codes = std::collections::HashSet::new();
        for (err, status, code) in taxonomy_cases() {
            assert_eq!(proxy_error_reason_code(&err), code);
            assert_eq!(proxy_error_http_status(&err), status);
            assert!(codes.insert(code), "duplicate reason code {code}");
        }
    }

    #[test]
    fn proxy_error_taxonomy_negative_n1_loadlinker_class_invalid_rebuild_400() {
        // Forced unsafe passthrough octets → composition 400 before dial (not 502).
        let mut req = empty_request(Method::GET);
        req.query_params = param_vec(&[("q", "a b")]);
        req.raw_query = Some("q=a b".to_string());
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/p");
        assert_eq!(res.status, 400, "Loadlinker-class must not be 502");
        assert_eq!(res.body["reason"], "unsafe_raw_query");
        assert_eq!(res.body["error"], "Bad Request");
        assert!(res.body["message"]
            .as_str()
            .unwrap()
            .starts_with("invalid path:"));
    }

    #[test]
    fn proxy_error_taxonomy_negative_n2_missing_path_param_400() {
        let req = empty_request(Method::GET);
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/items/{id}");
        assert_eq!(res.status, 400);
        assert_eq!(res.body["reason"], "unresolved_path_param");
        assert_ne!(res.status, 502);
    }

    #[test]
    fn proxy_error_taxonomy_negative_n3_invalid_uri_composition_400() {
        let err = ProxyError::invalid_path(ProxyPathReason::InvalidUri);
        let res = proxy_error_response(&err);
        assert_eq!(res.status, 400);
        assert_eq!(res.body["reason"], "invalid_uri");
        assert_eq!(res.body["message"], "invalid path: invalid uri character");
    }

    #[test]
    fn proxy_error_taxonomy_negative_n4_path_reason_required() {
        // Every InvalidPath carries ProxyPathReason; codes are non-empty.
        for reason in [
            ProxyPathReason::InvalidUri,
            ProxyPathReason::UnresolvedPathParam,
            ProxyPathReason::PathContainsQuestion,
            ProxyPathReason::UnsafeRawQuery,
            ProxyPathReason::UnsupportedQueryStyle,
        ] {
            let err = ProxyError::InvalidPath { reason };
            assert!(!proxy_error_reason_code(&err).is_empty());
            assert!(!reason.display_detail().is_empty());
        }
    }

    #[test]
    fn proxy_error_taxonomy_negative_n5_display_no_full_uri_leak() {
        let err = ProxyError::invalid_path(ProxyPathReason::InvalidUri);
        let s = err.to_string();
        assert!(!s.contains("/api/"));
        assert!(!s.contains('?'));
        assert!(!s.contains("secret"));
        assert!(!s.contains("Bearer"));
        // Length-only for overlong (no target body).
        let long = ProxyError::RequestTargetTooLong {
            len: 9000,
            max: 8192,
        };
        assert_eq!(long.to_string(), "request-target too long: 9000 > 8192");
    }

    #[test]
    fn proxy_error_taxonomy_negative_n6_table_driven_status_mapping() {
        for (err, expected_status, expected_reason) in taxonomy_cases() {
            assert_eq!(
                proxy_error_http_status(&err),
                expected_status,
                "status mismatch for {err:?}"
            );
            assert_eq!(
                proxy_error_reason_code(&err),
                expected_reason,
                "reason mismatch for {err:?}"
            );
            let res = proxy_error_response(&err);
            assert_eq!(res.status, expected_status);
            assert_eq!(res.body["reason"], expected_reason);
            assert_eq!(res.body["error"], proxy_error_title(expected_status));
        }
    }

    #[test]
    fn proxy_error_taxonomy_negative_n7_error_path_no_panic() {
        for (err, _, _) in taxonomy_cases() {
            let _ = err.to_string();
            let _ = proxy_error_response(&err);
            let _ = classify_transport_error(ProxyTransportKind::Request, "timed out");
        }
    }

    #[test]
    fn proxy_error_taxonomy_negative_n8_body_json_shape() {
        let res = proxy_error_response(&ProxyError::invalid_path(ProxyPathReason::InvalidUri));
        assert!(res.body.get("error").and_then(|v| v.as_str()).is_some());
        assert!(res.body.get("reason").and_then(|v| v.as_str()).is_some());
        assert!(res.body.get("message").and_then(|v| v.as_str()).is_some());
        assert_eq!(res.body.as_object().map(|o| o.len()), Some(3));
    }

    // --- Story 11.3 — PROXY QUERY + body ---

    #[derive(Default, Clone, Debug)]
    struct CapturedDownstream {
        method: String,
        content_type: Option<String>,
        body: Vec<u8>,
        /// True if the client-supplied `Connection: keep-alive` was forwarded (must be false).
        saw_forwarded_keep_alive: bool,
        saw_request_id: bool,
        hosts: Vec<String>,
    }

    fn spawn_capture_downstream() -> (u16, Arc<Mutex<Option<CapturedDownstream>>>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(false).expect("blocking accept");
        let port = listener.local_addr().unwrap().port();
        let captured = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&captured);
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                match stream.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if let Some(end) = find_http_header_end(&buf) {
                            let headers = &buf[..end];
                            let content_length = parse_content_length(headers).unwrap_or(0);
                            if buf.len() >= end + content_length {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            let parsed = parse_captured_http(&buf);
            *slot.lock().unwrap() = Some(parsed);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
            );
        });
        (port, captured)
    }

    fn find_http_header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
    }

    fn parse_content_length(headers: &[u8]) -> Option<usize> {
        let text = std::str::from_utf8(headers).ok()?;
        for line in text.lines() {
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-length:") {
                return rest.trim().parse().ok();
            }
        }
        None
    }

    fn parse_captured_http(buf: &[u8]) -> CapturedDownstream {
        let end = find_http_header_end(buf).unwrap_or(buf.len());
        let header_text = String::from_utf8_lossy(&buf[..end]);
        let mut lines = header_text.lines();
        let request_line = lines.next().unwrap_or("");
        let method = request_line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        let mut content_type = None;
        let mut saw_forwarded_keep_alive = false;
        let mut saw_request_id = false;
        let mut hosts = Vec::new();
        for line in lines {
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-type:") {
                content_type = Some(rest.trim().to_string());
            }
            if lower.starts_with("connection:") && lower.contains("keep-alive") {
                saw_forwarded_keep_alive = true;
            }
            if lower.starts_with("x-request-id:") {
                saw_request_id = true;
            }
            if let Some(rest) = lower.strip_prefix("host:") {
                hosts.push(rest.trim().to_string());
            }
        }
        let body = buf.get(end..).unwrap_or(&[]).to_vec();
        CapturedDownstream {
            method,
            content_type,
            body,
            saw_forwarded_keep_alive,
            saw_request_id,
            hosts,
        }
    }

    fn query_proxy_request(body: Option<Value>, headers: &[(&str, &str)]) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel();
        let mut hdrs = HeaderVec::new();
        for (k, v) in headers {
            hdrs.push((Arc::from(*k), (*v).to_string()));
        }
        HandlerRequest {
            request_id: RequestId::new(),
            method: crate::http::method_query(),
            path: "/search".to_string(),
            handler_name: "query_proxy_test".to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            raw_query: None,
            headers: hdrs,
            cookies: HeaderVec::new(),
            body,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
            span: tracing::Span::none(),
        }
    }

    /// P1 / P4 / P5 / N5 / N8 — BFF QUERY forwards method + same body bytes; not GET/POST.
    #[test]
    fn query_proxy_positive_p1_forwards_query_method_and_body() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let (port, captured) = spawn_capture_downstream();
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());

        let body = serde_json::json!({"q":"South Africa","limit":2});
        let expected = serde_json::to_vec(&body).unwrap();
        let req = query_proxy_request(
            Some(body),
            &[
                ("content-type", "application/json"),
                ("connection", "keep-alive"),
                ("x-request-id", "req-11-3"),
            ],
        );
        let res = proxy_untyped(&req, "127.0.0.1", "/search");
        assert_eq!(res.status, 200, "P5 2xx from mock: {:?}", res.body);

        let got = captured
            .lock()
            .unwrap()
            .clone()
            .expect("downstream should receive request");
        assert_eq!(got.method, "QUERY", "N8 must not mis-map to GET/POST");
        assert_eq!(got.body, expected, "P1/N5 same body bytes");
        assert_eq!(
            got.content_type.as_deref(),
            Some("application/json"),
            "P4 Content-Type preserved"
        );
        assert!(
            !got.saw_forwarded_keep_alive,
            "P3 Connection: keep-alive must not be forwarded"
        );
        assert!(got.saw_request_id, "P3 end-to-end headers still forward");

        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    /// The downstream sees exactly one Host naming itself, never the client's.
    #[test]
    fn proxy_sends_the_downstream_host_not_the_clients() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let (port, captured) = spawn_capture_downstream();
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());

        let req = query_proxy_request(None, &[("host", "platform.example.com")]);
        let res = proxy_untyped(&req, "127.0.0.1", "/search");
        assert_eq!(res.status, 200, "2xx from mock: {:?}", res.body);

        let got = captured
            .lock()
            .unwrap()
            .clone()
            .expect("downstream should receive request");
        assert_eq!(got.hosts, vec![format!("127.0.0.1:{port}")]);

        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    /// A client request without Host (HTTP/1.0, internal callers) still yields one downstream Host.
    #[test]
    fn proxy_sends_the_downstream_host_when_the_client_sent_none() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let (port, captured) = spawn_capture_downstream();
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());

        let req = query_proxy_request(None, &[("x-request-id", "req-host-1")]);
        let res = proxy_untyped(&req, "127.0.0.1", "/search");
        assert_eq!(res.status, 200, "2xx from mock: {:?}", res.body);

        let got = captured
            .lock()
            .unwrap()
            .clone()
            .expect("downstream should receive request");
        assert_eq!(got.hosts, vec![format!("127.0.0.1:{port}")]);

        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    /// The Host the proxy builds for a namespaced Service is its cluster DNS name and port.
    #[test]
    fn downstream_host_header_for_a_namespaced_service() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("POD_NAMESPACE", "pw");
        std::env::remove_var("SERVICE_HTTP_PORT");
        let value = crate::http::host_header_value(
            &downstream_host("platform-metrics"),
            downstream_http_port(),
            80,
        );
        assert_eq!(value, "platform-metrics.pw.svc.cluster.local:8080");
        std::env::remove_var("POD_NAMESPACE");
    }

    /// P2 / N6 — legacy http 0.2 Method bridge accepts QUERY.
    #[test]
    fn query_proxy_positive_p2_legacy_method_from_bytes() {
        let m = http_legacy::Method::from_bytes(b"QUERY").expect("N6 QUERY token");
        assert_eq!(m.as_str(), "QUERY");
        assert_eq!(
            http_legacy::Method::from_bytes(crate::http::method_query().as_str().as_bytes())
                .unwrap()
                .as_str(),
            "QUERY"
        );
    }

    /// N1 — invalid request-target on QUERY still composition 400 (not 502).
    #[test]
    fn query_proxy_negative_n1_invalid_target_is_400() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mut req = query_proxy_request(Some(serde_json::json!({"q":"x"})), &[]);
        req.query_params = param_vec(&[("q", "South Africa")]);
        // Force rebuild of unencoded space via mismatched raw → composition InvalidUri path
        // (same Loadlinker class as GET).
        let res = proxy_untyped(&req, "127.0.0.1", "/p?bad");
        // path template containing `?` → PathContainsQuestion → 400
        assert_eq!(res.status, 400, "N1 composition not 502: {:?}", res.body);
        assert_ne!(res.status, 502);
    }

    /// N2 — DNS/connect failure remains 502 (not composition).
    #[test]
    fn query_proxy_negative_n2_dns_is_502() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("POD_NAMESPACE", "logistics");
        std::env::set_var("SERVICE_HTTP_PORT", "8080");
        let req = query_proxy_request(Some(serde_json::json!({"q":"x"})), &[]);
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/search");
        assert_eq!(res.status, 502);
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    /// N3 — empty body when none attached: proxy does not panic (required-body 400 is service).
    #[test]
    fn query_proxy_negative_n3_empty_body_no_panic() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let (port, captured) = spawn_capture_downstream();
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
        let req = query_proxy_request(None, &[("content-type", "application/json")]);
        let res = proxy_untyped(&req, "127.0.0.1", "/search");
        assert_eq!(res.status, 200);
        let got = captured.lock().unwrap().clone().unwrap();
        assert_eq!(got.method, "QUERY");
        assert!(got.body.is_empty(), "N3/N5 empty body not fabricated");
        std::env::remove_var("SERVICE_HTTP_PORT");
    }

    /// N4 — oversized *response* body → BodyTooLarge → 502; no panic.
    #[test]
    fn query_proxy_negative_n4_oversized_response_mapped() {
        assert_eq!(proxy_error_http_status(&ProxyError::BodyTooLarge), 502);
        let _ = proxy_error_response(&ProxyError::BodyTooLarge);
    }

    /// N7 — forward error paths do not panic.
    #[test]
    fn query_proxy_negative_n7_no_panic_on_errors() {
        let _lock = ENV_LOCK.lock().unwrap();
        let req = query_proxy_request(Some(serde_json::json!({"q":"x"})), &[]);
        let _ = proxy_untyped(&req, "no-such-service-xyz.invalid", "/search");
        let _ = proxy_untyped(&req, "127.0.0.1", "/p?bad");
    }

    #[test]
    fn sse_frame_end_finds_the_blank_line() {
        assert_eq!(sse_frame_end(b"data: a\n\nrest"), Some(9));
        assert_eq!(sse_frame_end(b"data: a\r\n\r\n"), Some(11));
        assert_eq!(sse_frame_end(b"data: a\r\rx"), Some(9));
        assert_eq!(
            sse_frame_end(b"data: {}\nevent: alert\nid: 7\n\n"),
            Some(29)
        );
        assert_eq!(sse_frame_end(b"data: a\n"), None);
        assert_eq!(sse_frame_end(b"data: a\r"), None);
        // The empty line's CR is the last byte: the frame is complete.
        assert_eq!(sse_frame_end(b"data: a\n\r"), Some(9));
        assert_eq!(sse_frame_end(b"data: a\r\r"), Some(9));
    }

    /// Reads one chunk per call, as a socket would hand them over.
    struct Chunks(Vec<&'static [u8]>);

    impl Read for Chunks {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() {
                return Ok(0);
            }
            let c = self.0.remove(0);
            buf[..c.len()].copy_from_slice(c);
            Ok(c.len())
        }
    }

    fn relayed(chunks: Vec<&'static [u8]>) -> Vec<String> {
        let (tx, rx) = crate::sse::channel();
        relay_frames(&mut Chunks(chunks), &tx);
        drop(tx);
        std::iter::from_fn(|| rx.recv_frame()).collect()
    }

    #[test]
    fn a_cr_terminated_last_event_is_delivered() {
        assert_eq!(relayed(vec![b"data: a\r\r"]), vec!["data: a\r\r"]);
    }

    #[test]
    fn a_crlf_split_across_reads_is_not_an_extra_line() {
        assert_eq!(
            relayed(vec![b"data: a\r\n\r", b"\ndata: b\r\n\r\n"]),
            vec!["data: a\r\n\r", "data: b\r\n\r\n"]
        );
    }

    #[test]
    fn a_client_that_falls_behind_ends_the_relay() {
        let (tx, rx) = crate::sse::channel_limited(2, usize::MAX);
        relay_frames(
            &mut Chunks(vec![b"data: 1\n\ndata: 2\n\ndata: 3\n\ndata: 4\n\n"]),
            &tx,
        );
        drop(tx);
        let got: Vec<String> = std::iter::from_fn(|| rx.recv_frame()).collect();
        assert_eq!(got, vec!["data: 1\n\n", "data: 2\n\n"]);
    }

    /// One downstream that answers with `head`, then writes `chunks` as HTTP chunks.
    fn spawn_sse_downstream(head: &'static str, chunks: Vec<&'static str>) -> u16 {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while find_http_header_end(&buf).is_none() {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }
            let _ = stream.write_all(head.as_bytes());
            for c in chunks {
                let _ = stream.write_all(format!("{:x}\r\n{c}\r\n", c.len()).as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let _ = stream.write_all(b"0\r\n\r\n");
        });
        port
    }

    #[test]
    fn proxy_sse_relays_frames_with_event_names_and_ids() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let port = spawn_sse_downstream(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nX-Stream: alerts\r\n\r\n",
            vec![
                "data: {\"cursor\":1}\nevent: hello\n\n",
                "data: {\"id\":\"a\"}\neve",
                "nt: alert\nid: 7\n\ndata: \n: ping\n\n",
            ],
        );
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
        let res = proxy_sse(&empty_request(Method::GET), "127.0.0.1", "/stream");
        std::env::remove_var("SERVICE_HTTP_PORT");

        assert_eq!(res.status, 200);
        assert!(res
            .headers
            .iter()
            .any(|(k, v)| k.as_ref() == "x-stream" && v == "alerts"));
        let rx = res.sse.expect("a live stream");
        let mut frames = Vec::new();
        while let Some(f) = rx.recv_frame() {
            frames.push(f);
        }
        assert_eq!(
            frames,
            vec![
                "data: {\"cursor\":1}\nevent: hello\n\n",
                "data: {\"id\":\"a\"}\nevent: alert\nid: 7\n\n",
                "data: \n: ping\n\n",
            ]
        );
    }

    #[test]
    fn proxy_sse_asks_for_and_requires_an_uncompressed_stream() {
        use std::io::{Read as _, Write};
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(String::new()));
        let slot = Arc::clone(&seen);
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while find_http_header_end(&buf).is_none() {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }
            *slot.lock().unwrap() = String::from_utf8_lossy(&buf).to_ascii_lowercase();
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n",
            );
        });
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
        let mut req = empty_request(Method::GET);
        req.headers
            .push((Arc::from("accept-encoding"), "gzip, br".to_string()));
        let res = proxy_sse(&req, "127.0.0.1", "/stream");
        std::env::remove_var("SERVICE_HTTP_PORT");

        let sent = seen.lock().unwrap().clone();
        assert!(sent.contains("accept-encoding: identity"), "{sent}");
        assert!(!sent.contains("gzip"), "{sent}");
        assert!(res.sse.is_none());
        assert_eq!(res.status, proxy_error_http_status(&ProxyError::Response));
    }

    #[test]
    fn passthrough_body_keeps_only_2xx_json_text() {
        let json = Some("application/json; charset=utf-8");
        assert_eq!(
            passthrough_body(b"[1, 2]".to_vec(), 200, json),
            Ok("[1, 2]".to_string())
        );
        assert_eq!(
            passthrough_body(b"{}".to_vec(), 201, Some("application/problem+json")),
            Ok("{}".to_string())
        );
        // Errors, other media types, empty bodies and invalid UTF-8 come back as bytes.
        assert_eq!(
            passthrough_body(b"{}".to_vec(), 404, json),
            Err(b"{}".to_vec())
        );
        assert_eq!(
            passthrough_body(b"hi".to_vec(), 200, Some("text/plain")),
            Err(b"hi".to_vec())
        );
        assert_eq!(
            passthrough_body(b"{}".to_vec(), 200, None),
            Err(b"{}".to_vec())
        );
        assert_eq!(passthrough_body(Vec::new(), 200, json), Err(Vec::new()));
        assert_eq!(
            passthrough_body(vec![0xff, 0xfe], 200, json),
            Err(vec![0xff, 0xfe])
        );
    }

    #[test]
    fn proxy_untyped_parses_a_2xx_json_body_when_pass_through_is_off() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let (port, _captured) = spawn_capture_downstream();
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
        let res = proxy_untyped_inner(&empty_request(Method::GET), "127.0.0.1", "/search", false)
            .expect("proxied");
        std::env::remove_var("SERVICE_HTTP_PORT");

        assert_eq!(res.body, serde_json::json!({"ok": true}));
        assert_eq!(
            res.get_header(crate::server::response::RAW_BODY_ENCODING_HEADER),
            None
        );
    }

    #[test]
    fn proxy_untyped_passes_a_2xx_json_body_through_as_text() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let (port, _captured) = spawn_capture_downstream();
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
        let res = proxy_untyped_inner(&empty_request(Method::GET), "127.0.0.1", "/search", true)
            .expect("proxied");
        std::env::remove_var("SERVICE_HTTP_PORT");

        assert_eq!(res.status, 200);
        assert_eq!(res.body, Value::String("{\"ok\":true}".to_string()));
        assert_eq!(
            res.get_header(crate::server::response::RAW_BODY_ENCODING_HEADER),
            Some(crate::server::response::RAW_BODY_ENCODING_JSON)
        );
    }

    #[test]
    fn proxy_sse_answers_a_downstream_error_as_is() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POD_NAMESPACE");
        let port = spawn_sse_downstream(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
            vec!["{\"error\":\"unauthorized\"}"],
        );
        std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
        let res = proxy_sse(&empty_request(Method::GET), "127.0.0.1", "/stream");
        std::env::remove_var("SERVICE_HTTP_PORT");

        assert_eq!(res.status, 401);
        assert!(res.sse.is_none());
        assert_eq!(res.body["error"], "unauthorized");
    }
}
