//! OpenTelemetry instrumentation of the pooled outbound client: a decorator around
//! [`may_minihttp::client::Client`] used by `fetch_*` and [`super::proxy_untyped`].
//!
//! Each call is one **client span** (`http_client`, exported as `{METHOD} {route}`, `otel.kind =
//! client`) under the caller's context: the handler request's span when the caller has one, else
//! the coroutine context ([`may_tracing::current`]). Attributes are low-cardinality only:
//! `http.request.method`, `server.address`, `server.port`, `url.scheme`, `http.route` (the path
//! template, when the caller knows it; never a path with ids or a query string),
//! `http.response.status_code`, `error.type`, `http.request.resend_count` and
//! `may_minihttp.connection_reused` (whether the pool reused a connection).
//!
//! The span is handed to may_minihttp as its request span (`RequestBuilder::request_span`, the
//! fork's `tracing` feature): the client records status, error, resend count and reuse on it and
//! opens its own spans under it: `http_client_connect` (INFO) for a new connection, and
//! `http_client_write` / `http_client_wait` (time to first byte) / `http_client_body` (DEBUG).
//!
//! The W3C `traceparent` / `tracestate` of the client span go on the request, so the server's span
//! is a child of this call, once per call: a retry the pool makes on another connection is the
//! same client span (its resend count says so). A `traceparent` the caller set itself is kept; a
//! forwarded one (the proxy's) is replaced.
//!
//! Calls through the fetch pool are also recorded in `http_client_request_duration_seconds`
//! ([`super::client_metrics`]); proxied calls are recorded there by the proxy, with its own error
//! reasons.
//!
//! **Disabled tracing costs nothing extra.** When the span is disabled (no subscriber, or a filter
//! that excludes `brrtrouter::http_client`), no field is evaluated, no context is read, no header
//! is added and nothing is allocated for tracing.
//!
//! Why not may_minihttp's `Interceptor`: it runs per *attempt* and is static per client, while the
//! trace context belongs to the logical call (one client span across the pool's retries) and is
//! per call. Setting the headers once on the request is exactly that; the interceptor stays free
//! for per-attempt needs.

use std::io::{self, Read};
use std::time::{Duration, Instant};

use http_legacy::{HeaderMap, HeaderName, HeaderValue, Method};
use may_minihttp::client::{Client, ClientError, ClientStats, RequestBuilder, StreamingResponse};
use opentelemetry::propagation::{Injector, TextMapPropagator};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing::field::Empty;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// The target of the client span; filter on it to turn the spans off.
pub const TARGET: &str = "brrtrouter::http_client";

/// A pooled client whose calls are traced (and, for the fetch pool, measured).
pub struct TracedClient {
    client: Client,
    record_metrics: bool,
}

impl TracedClient {
    /// `record_metrics`: record each call in `http_client_request_duration_seconds` (the proxy
    /// records its calls itself).
    #[must_use]
    pub fn new(client: Client, record_metrics: bool) -> Self {
        Self {
            client,
            record_metrics,
        }
    }

    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }

    #[must_use]
    pub fn stats(&self) -> ClientStats {
        self.client.stats()
    }

    /// Start a call; same contract as [`Client::request`].
    pub fn request<'a>(&'a self, method: Method, url: &'a str) -> io::Result<TracedRequest<'a>> {
        let builder = self.client.request(method.clone(), url)?;
        Ok(TracedRequest {
            owner: self,
            builder,
            method,
            url,
            trace_headers: TraceHeaders::default(),
            route: None,
            server_label: None,
            parent: None,
            forwarded_context: false,
        })
    }
}

/// One traced call being built. Mirrors [`RequestBuilder`]'s methods, plus the call's route.
pub struct TracedRequest<'a> {
    owner: &'a TracedClient,
    builder: RequestBuilder,
    method: Method,
    url: &'a str,
    /// `traceparent` / `tracestate` set by the caller, held back until the call's own context is
    /// known; every other header goes straight into the request.
    trace_headers: TraceHeaders,
    route: Option<&'a str>,
    server_label: Option<&'a str>,
    parent: Option<&'a Span>,
    forwarded_context: bool,
}

impl<'a> TracedRequest<'a> {
    #[must_use]
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        if name == TRACEPARENT {
            self.trace_headers.parent = Some(value);
        } else if name == TRACESTATE {
            self.trace_headers.state = Some(value);
        } else {
            self.builder = self.builder.header(name, value);
        }
        self
    }

    #[must_use]
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.builder = self.builder.body(body);
        self
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.builder = self.builder.timeout(timeout);
        self
    }

    /// The path template of what is called (`/orders/{id}`), for `http.route` and the span name.
    #[must_use]
    pub fn route(mut self, route: &'a str) -> Self {
        self.route = Some(route);
        self
    }

    /// `server_address` in the duration histogram (default: the URL's host).
    #[must_use]
    pub fn server_label(mut self, label: &'a str) -> Self {
        self.server_label = Some(label);
        self
    }

    /// The span this call is a child of (default: the coroutine context).
    #[must_use]
    pub fn parent(mut self, parent: &'a Span) -> Self {
        if !parent.is_none() {
            self.parent = Some(parent);
        }
        self
    }

    /// The `traceparent` / `tracestate` among the headers were forwarded from an earlier hop
    /// (a proxied request): this call's span replaces them. Without this, a `traceparent` the
    /// caller set is kept.
    #[must_use]
    pub fn forwarded_trace_context(mut self) -> Self {
        self.forwarded_context = true;
        self
    }

    /// Send; the response's body is read through [`TracedResponse`], which ends the span (and
    /// records the call) when it is dropped.
    pub fn send_streaming(self) -> io::Result<TracedResponse> {
        let started = Instant::now();
        let origin = Origin::of(self.url);
        let current;
        let parent = match self.parent {
            Some(parent) => parent,
            None => {
                current = may_tracing::current();
                &current
            }
        };
        let span = tracing::span!(
            target: TARGET,
            parent: parent,
            tracing::Level::INFO,
            "http_client",
            otel.name = Empty,
            otel.kind = "client",
            otel.status_code = Empty,
            "http.request.method" = self.method.as_str(),
            "server.address" = origin.host,
            "server.port" = i64::from(origin.port),
            "url.scheme" = origin.scheme,
            "http.route" = Empty,
            "http.response.status_code" = Empty,
            "error.type" = Empty,
            "http.request.resend_count" = Empty,
            "may_minihttp.connection_reused" = Empty,
        );
        let mut trace_headers = self.trace_headers;
        if !span.is_disabled() {
            name_span(&span, &self.method, self.route);
            if self.forwarded_context || trace_headers.parent.is_none() {
                inject_context(&span, &mut trace_headers);
            }
        }
        let mut builder = self.builder.request_span(&span);
        if let Some(value) = trace_headers.parent {
            builder = builder.header(TRACEPARENT, value);
        }
        if let Some(value) = trace_headers.state {
            builder = builder.header(TRACESTATE, value);
        }
        let metrics = self.owner.record_metrics.then(|| Call {
            server: self
                .server_label
                .map_or_else(|| origin.host.to_owned(), str::to_owned),
            method: self.method,
            route: self.route.map(str::to_owned),
            started,
        });
        match builder.send_streaming() {
            Ok(response) => {
                let status = response.status().as_u16();
                // As an integer attribute (may_minihttp records it too, as a u16).
                span.record("http.response.status_code", i64::from(status));
                if status >= 400 && !span.is_disabled() {
                    span.record("otel.status_code", "ERROR");
                    span.record("error.type", status_text(status));
                }
                Ok(TracedResponse {
                    response,
                    span,
                    metrics,
                    status,
                })
            }
            Err(error) => {
                if !span.is_disabled() {
                    span.record("otel.status_code", "ERROR");
                }
                let error = ClientError::from(error);
                if let Some(call) = metrics {
                    call.record(None, Some(error.kind().as_str()));
                }
                Err(error.into_io_error())
            }
        }
    }
}

/// A traced call's response. Reading the body reads the pooled connection; dropping the response
/// ends the client span and records the call.
pub struct TracedResponse {
    response: StreamingResponse,
    span: Span,
    metrics: Option<Call>,
    status: u16,
}

impl TracedResponse {
    #[must_use]
    pub fn status(&self) -> http_legacy::StatusCode {
        self.response.status()
    }

    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        self.response.headers()
    }

    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.response.headers_mut()
    }

    #[must_use]
    pub fn reused_connection(&self) -> bool {
        self.response.reused_connection()
    }

    /// The client span of this call.
    #[must_use]
    pub fn span(&self) -> &Span {
        &self.span
    }
}

impl Read for TracedResponse {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.response.read(buf)
    }
}

impl Drop for TracedResponse {
    fn drop(&mut self) {
        if let Some(call) = self.metrics.take() {
            call.record(Some(self.status), None);
        }
    }
}

/// What the duration histogram needs of one call.
struct Call {
    server: String,
    method: Method,
    route: Option<String>,
    started: Instant,
}

impl Call {
    fn record(self, status: Option<u16>, error: Option<&str>) {
        super::client_metrics::record(
            &self.server,
            self.method.as_str(),
            self.route.as_deref(),
            status,
            error,
            self.started.elapsed(),
        );
    }
}

/// Scheme, host and port of an absolute URL, borrowed (no parse into a `Url`).
struct Origin<'a> {
    scheme: &'a str,
    host: &'a str,
    port: u16,
}

impl<'a> Origin<'a> {
    fn of(url: &'a str) -> Self {
        let (scheme, rest) = url.split_once("://").unwrap_or(("http", url));
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let authority = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let default_port = if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        };
        // `[v6]:port`, `host:port`, or a bare host.
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !port.contains(']') => {
                (host, port.parse().unwrap_or(default_port))
            }
            _ => (authority, default_port),
        };
        Self {
            scheme,
            host: host.trim_start_matches('[').trim_end_matches(']'),
            port,
        }
    }
}

/// `{METHOD} {route}` as the exported span name (the semantic convention's `{method} {target}`),
/// or just the method when the route is unknown.
fn name_span(span: &Span, method: &Method, route: Option<&str>) {
    match route {
        Some(route) => {
            span.record("http.route", route);
            let name = format!("{} {route}", method.as_str());
            span.record("otel.name", name.as_str());
        }
        None => {
            span.record("otel.name", method.as_str());
        }
    }
}

fn status_text(status: u16) -> &'static str {
    match status {
        400 => "400",
        401 => "401",
        403 => "403",
        404 => "404",
        408 => "408",
        409 => "409",
        422 => "422",
        429 => "429",
        500 => "500",
        502 => "502",
        503 => "503",
        504 => "504",
        s if s < 500 => "4xx",
        _ => "5xx",
    }
}

const TRACEPARENT: HeaderName = HeaderName::from_static("traceparent");
const TRACESTATE: HeaderName = HeaderName::from_static("tracestate");

/// The W3C trace-context headers of one call.
#[derive(Default)]
struct TraceHeaders {
    parent: Option<HeaderValue>,
    state: Option<HeaderValue>,
}

/// The propagator writes here, replacing what the caller set (a forwarded `traceparent` names
/// the previous hop's span, not this call). A context with no `tracestate` drops a forwarded one.
impl Injector for TraceHeaders {
    fn set(&mut self, key: &str, value: String) {
        if key == "tracestate" && value.is_empty() {
            self.state = None;
            return;
        }
        let Ok(value) = HeaderValue::try_from(value) else {
            return;
        };
        match key {
            "traceparent" => {
                self.parent = Some(value);
                self.state = None;
            }
            "tracestate" => self.state = Some(value),
            _ => {}
        }
    }
}

/// The client span's W3C trace context into `headers`. Nothing changes when the span has no valid
/// OpenTelemetry context (no OpenTelemetry layer).
fn inject_context(span: &Span, headers: &mut TraceHeaders) {
    let cx = span.context();
    TraceContextPropagator::new().inject_context(&cx, headers);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_read_without_parsing_the_url() {
        let o = Origin::of("http://orders.ns.svc:8080/orders/12?x=1");
        assert_eq!((o.scheme, o.host, o.port), ("http", "orders.ns.svc", 8080));
        let o = Origin::of("https://idp.example.com/.well-known/jwks.json");
        assert_eq!(
            (o.scheme, o.host, o.port),
            ("https", "idp.example.com", 443)
        );
        let o = Origin::of("http://user:pw@[::1]:9000/");
        assert_eq!((o.scheme, o.host, o.port), ("http", "::1", 9000));
        let o = Origin::of("http://[::1]/");
        assert_eq!((o.host, o.port), ("::1", 80));
    }

    #[test]
    fn statuses_map_to_static_error_types() {
        assert_eq!(status_text(503), "503");
        assert_eq!(status_text(418), "4xx");
        assert_eq!(status_text(599), "5xx");
    }
}
