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

const DEFAULT_DOWNSTREAM_PORT: u16 = 8080;
const DEFAULT_PROXY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PROXY_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Errors from the BFF downstream proxy layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyError {
    InvalidMethod(String),
    InvalidPath(String),
    Dns(String),
    Connect(String),
    Request(String),
    Response(String),
    BodySerialize(String),
    BodyTooLarge,
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMethod(msg) => write!(f, "invalid method: {msg}"),
            Self::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Self::Dns(msg) => write!(f, "dns: {msg}"),
            Self::Connect(msg) => write!(f, "connect: {msg}"),
            Self::Request(msg) => write!(f, "request: {msg}"),
            Self::Response(msg) => write!(f, "response: {msg}"),
            Self::BodySerialize(msg) => write!(f, "body serialize: {msg}"),
            Self::BodyTooLarge => write!(f, "response body exceeds limit"),
        }
    }
}

impl std::error::Error for ProxyError {}

/// Resolve `{param}` placeholders and append query string.
#[must_use]
pub fn resolve_path_template(
    path_template: &str,
    path_params: &ParamVec,
    query_params: &ParamVec,
) -> String {
    let mut resolved_path = path_template.to_string();
    for (k, v) in path_params {
        let needle = format!("{{{k}}}");
        resolved_path = resolved_path.replace(&needle, v.as_ref());
    }

    if !query_params.is_empty() {
        let mut qs = String::new();
        for (i, (k, v)) in query_params.iter().enumerate() {
            if i > 0 {
                qs.push('&');
            } else {
                qs.push('?');
            }
            qs.push_str(k.as_ref());
            qs.push('=');
            qs.push_str(v.as_ref());
        }
        resolved_path.push_str(&qs);
    }
    resolved_path
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
        .map_err(|e| ProxyError::Response(e.to_string()))?;
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
        Err(e) => HandlerResponse::error(502, &e.to_string()),
    }
}

fn proxy_untyped_inner(
    req: &HandlerRequest,
    downstream_service: &str,
    path_template: &str,
) -> Result<HandlerResponse, ProxyError> {
    let resolved_path = resolve_path_template(path_template, &req.path_params, &req.query_params);
    let host = downstream_host(downstream_service);
    let port = downstream_http_port();

    let target_ip = ToSocketAddrs::to_socket_addrs(&(host.as_str(), port))
        .map_err(|e| ProxyError::Dns(e.to_string()))?
        .next()
        .ok_or_else(|| ProxyError::Dns("DNS resolution empty".to_string()))?;

    let uri: Uri = resolved_path
        .parse::<Uri>()
        .map_err(|e: http_legacy::uri::InvalidUri| ProxyError::InvalidPath(e.to_string()))?;

    let method = Method::from_bytes(req.method.as_str().as_bytes())
        .map_err(|e| ProxyError::InvalidMethod(e.to_string()))?;

    let mut client =
        HttpClient::connect(target_ip).map_err(|e| ProxyError::Connect(e.to_string()))?;
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
        let body_bytes =
            serde_json::to_vec(body_json).map_err(|e| ProxyError::BodySerialize(e.to_string()))?;
        proxy_req
            .send(&body_bytes)
            .map_err(|e| ProxyError::Request(e.to_string()))?;
    }

    let mut rsp = client
        .send_request(proxy_req)
        .map_err(|e| ProxyError::Request(e.to_string()))?;

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
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
        }
    }

    #[test]
    fn resolve_path_template_substitutes_params_and_query() {
        let mut path_params = ParamVec::new();
        path_params.push((Arc::from("id"), "abc".to_string()));
        let mut query_params = ParamVec::new();
        query_params.push((Arc::from("limit"), "10".to_string()));

        let path =
            resolve_path_template("/api/v1/fleet/vehicles/{id}", &path_params, &query_params);
        assert_eq!(path, "/api/v1/fleet/vehicles/abc?limit=10");
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
}
