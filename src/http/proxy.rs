//! BFF downstream HTTP proxy — Kubernetes Service-name routing.
//!
//! Replaces generated inline proxy logic in `templates/controller.rs.txt`.
//! Downstream targets are resolved by OpenAPI `x-service` (Kubernetes Service name)
//! and `HAULIAGE_SERVICE_HTTP_PORT` (default 8080). Each request opens a fresh
//! `may_minihttp` connection to avoid cross-service client reuse (FR-26).

use std::io::Read;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use http_legacy::{Method, Uri};
use may_minihttp::client::HttpClient;
use serde_json::Value;

use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use crate::router::ParamVec;
use crate::server::request::parse_query_params;
use crate::server::request_target::{max_request_target_octets, request_target_exceeds_limit};

const DEFAULT_DOWNSTREAM_PORT: u16 = 8080;
const DEFAULT_PROXY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PROXY_BODY_BYTES: usize = 16 * 1024 * 1024;

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
    std::env::var("HAULIAGE_SERVICE_HTTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_DOWNSTREAM_PORT)
}

/// Stable pool key for `(host, port)` — used in tests; runtime connects per request.
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
}

fn proxy_timeout() -> Duration {
    std::env::var("HAULIAGE_PROXY_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| Duration::from_secs(secs.max(1)))
        .unwrap_or(DEFAULT_PROXY_TIMEOUT)
}

fn read_bounded_body(reader: &mut impl Read, max_body: usize) -> Result<Vec<u8>, ProxyError> {
    let mut buf = Vec::new();
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
    match proxy_untyped_inner(req, downstream_service, path_template) {
        Ok(res) => res,
        Err(e) => proxy_error_response(&e),
    }
}

fn proxy_untyped_inner(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
) -> Result<HandlerResponse, ProxyError> {
    let resolved_path = resolve_downstream_target(
        path_template,
        &req.path_params,
        &req.query_params,
        req.raw_query.as_deref(),
    )?;
    let host = downstream_host(downstream_service);
    let port = downstream_http_port();

    let target_ip = ToSocketAddrs::to_socket_addrs(&(host.as_str(), port))
        .map_err(|e| classify_transport_error(ProxyTransportKind::Dns, &e.to_string()))?
        .next()
        .ok_or(ProxyError::Dns)?;

    // Decision B: validate both stacks; convert to legacy Uri only at this edge
    // (no full request-target embedded in error Display).
    let uri = crate::server::request_target::RequestTarget::try_from_path_query(resolved_path)
        .and_then(|t| t.to_legacy_uri())
        .map_err(|_e| ProxyError::invalid_path(ProxyPathReason::InvalidUri))?;

    let method = Method::from_bytes(req.method.as_str().as_bytes())
        .map_err(|_e| ProxyError::InvalidMethod)?;

    let mut client = HttpClient::connect(target_ip)
        .map_err(|e| classify_transport_error(ProxyTransportKind::Connect, &e.to_string()))?;
    client.set_timeout(Some(proxy_timeout()));

    let mut proxy_req = client.new_request(method, uri);

    for (hk, hv) in &req.headers {
        let name = hk.as_ref();
        if skip_forward_request_header(name) {
            continue;
        }
        if let (Ok(hname), Ok(hval)) = (
            http_legacy::header::HeaderName::from_bytes(name.as_bytes()),
            http_legacy::header::HeaderValue::from_str(hv.as_str()),
        ) {
            proxy_req.headers_mut().insert(hname, hval);
        }
    }

    if proxy_req
        .headers()
        .get(http_legacy::header::ACCEPT)
        .is_none()
    {
        if let Ok(safe_accept) = http_legacy::header::HeaderValue::from_str("application/json") {
            proxy_req
                .headers_mut()
                .insert(http_legacy::header::ACCEPT, safe_accept);
        }
    }

    if let Some(body_json) = &req.body {
        let body_bytes = serde_json::to_vec(body_json).map_err(|_e| ProxyError::BodySerialize)?;
        proxy_req
            .send(&body_bytes)
            .map_err(|e| classify_transport_error(ProxyTransportKind::Request, &e.to_string()))?;
    }

    let mut rsp = client
        .send_request(proxy_req)
        .map_err(|e| classify_transport_error(ProxyTransportKind::Request, &e.to_string()))?;

    let buf = read_bounded_body(&mut rsp, MAX_PROXY_BODY_BYTES)?;
    let content_type = rsp
        .headers()
        .get(http_legacy::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let body_json = response_body_value(&buf, content_type);
    let status = rsp.status().as_u16();

    let mut out_headers = HeaderVec::new();
    for (name, value) in rsp.headers().iter() {
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
        std::env::remove_var("HAULIAGE_SERVICE_HTTP_PORT");
        assert_eq!(downstream_http_port(), 8080);
    }

    #[test]
    fn downstream_http_port_reads_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("HAULIAGE_SERVICE_HTTP_PORT", "9090");
        assert_eq!(downstream_http_port(), 9090);
        std::env::remove_var("HAULIAGE_SERVICE_HTTP_PORT");
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
        std::env::set_var("HAULIAGE_SERVICE_HTTP_PORT", "8080");
        let req = empty_request(Method::GET);
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/health");
        assert_eq!(res.status, 502);
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("HAULIAGE_SERVICE_HTTP_PORT");
    }

    // --- Story 10.5 query passthrough ---

    #[test]
    fn resolve_downstream_positive_p1_passthrough_plus_and_pct2b() {
        let q = param_vec(&[("q", "a+b c")]);
        let raw = "q=a%2Bb+c";
        assert!(query_params_match_raw(raw, &q));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=a%2Bb+c");
    }

    #[test]
    fn resolve_downstream_positive_p2_passthrough_preserves_pct20() {
        let q = param_vec(&[("q", "South Africa")]);
        let raw = "q=South%20Africa";
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=South%20Africa");
        assert!(!t.contains('+'));
    }

    #[test]
    fn resolve_downstream_positive_p3_path_param_rebuild_path() {
        let path = param_vec(&[("id", "a b")]);
        let t = resolve_downstream_target("/items/{id}", &path, &ParamVec::new(), None).unwrap();
        assert_eq!(t, "/items/a%20b");
        assert_parseable_uri(&t);
    }

    #[test]
    fn resolve_downstream_positive_p4_path_sub_query_passthrough() {
        let path = param_vec(&[("id", "x")]);
        let q = param_vec(&[("q", "a+b c")]);
        let raw = "q=a%2Bb+c";
        let t = resolve_downstream_target("/items/{id}", &path, &q, Some(raw)).unwrap();
        assert_eq!(t, "/items/x?q=a%2Bb+c");
    }

    #[test]
    fn resolve_downstream_positive_p5_multi_param_passthrough() {
        let q = param_vec(&[("a", "1"), ("b", "2")]);
        let raw = "a=1&b=2";
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?a=1&b=2");
    }

    #[test]
    fn resolve_downstream_positive_p6_empty_query_no_spurious_qmark() {
        let t =
            resolve_downstream_target("/p", &ParamVec::new(), &ParamVec::new(), Some("")).unwrap();
        assert_eq!(t, "/p");
        let t2 = resolve_downstream_target("/p", &ParamVec::new(), &ParamVec::new(), None).unwrap();
        assert_eq!(t2, "/p");
    }

    #[test]
    fn resolve_downstream_positive_p7_rebuild_space_to_pct20() {
        let q = param_vec(&[("k", "South Africa")]);
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, None).unwrap();
        assert_eq!(t, "/p?k=South%20Africa");
        assert_parseable_uri(&t);
    }

    #[test]
    fn resolve_downstream_positive_p8_unmutated_selects_passthrough() {
        let q = param_vec(&[("q", "x")]);
        let raw = "q=x";
        assert!(query_params_match_raw(raw, &q));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=x");
    }

    #[test]
    fn resolve_downstream_negative_n1_raw_space_rejected() {
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
        let mutated = param_vec(&[("q", "new")]);
        let raw = "q=old";
        assert!(!query_params_match_raw(raw, &mutated));
        let t = resolve_downstream_target("/p", &ParamVec::new(), &mutated, Some(raw)).unwrap();
        assert_eq!(t, "/p?q=new");
        assert!(!t.contains("old"));
    }

    #[test]
    fn resolve_downstream_negative_n4_missing_path_param() {
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
        std::env::set_var("HAULIAGE_SERVICE_HTTP_PORT", "8080");
        let req = empty_request(Method::GET);
        let res = proxy_untyped(&req, "no-such-service-xyz.invalid", "/health");
        assert_eq!(res.status, 502);
        assert_eq!(res.body["reason"], "dns");
        assert_eq!(res.body["error"], "Bad Gateway");
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("HAULIAGE_SERVICE_HTTP_PORT");
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
}
