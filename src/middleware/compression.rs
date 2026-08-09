//! Response compression middleware (Epic 13.8).
//!
//! Opt-in gzip for eligible JSON / `text/*` responses. Default **off**.
//! Never compresses SSE (`text/event-stream`), images, or already-encoded bodies.
//!
//! Compressed payloads use the same raw-body wire path as [`crate::typed::HttpFile`]
//! (`x-brrtrouter-raw-encoding: base64`) so `Content-Length` stays correct.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::Value;

use super::{MetricsMiddleware, Middleware};
use crate::dispatcher::{HandlerRequest, HandlerResponse};
use crate::server::response::{RAW_BODY_ENCODING_BASE64, RAW_BODY_ENCODING_HEADER};

/// Default minimum uncompressed size (bytes) before gzip is attempted.
pub const DEFAULT_MIN_BYTES: usize = 256;

/// Configuration for [`CompressionMiddleware`].
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// When false, middleware is a no-op (default).
    pub enabled: bool,
    /// Skip compression when payload is smaller than this many bytes.
    pub min_bytes: usize,
    /// Gzip level (0–9). Default [`Compression::default`].
    pub level: u32,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_bytes: DEFAULT_MIN_BYTES,
            level: 6,
        }
    }
}

impl CompressionConfig {
    /// Enabled gzip with default threshold.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

/// Gzip response compression (opt-in).
pub struct CompressionMiddleware {
    config: CompressionConfig,
    compressed_responses: AtomicU64,
    compressed_bytes_in: AtomicU64,
    compressed_bytes_out: AtomicU64,
    metrics: Option<Arc<MetricsMiddleware>>,
}

impl CompressionMiddleware {
    #[must_use]
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            compressed_responses: AtomicU64::new(0),
            compressed_bytes_in: AtomicU64::new(0),
            compressed_bytes_out: AtomicU64::new(0),
            metrics: None,
        }
    }

    #[must_use]
    pub fn with_metrics_sink(mut self, metrics: Arc<MetricsMiddleware>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    #[must_use]
    pub fn compressed_responses(&self) -> u64 {
        self.compressed_responses.load(Ordering::Relaxed)
    }

    /// `true` if `Accept-Encoding` lists gzip with non-zero q.
    #[must_use]
    pub fn client_accepts_gzip(accept_encoding: Option<&str>) -> bool {
        let Some(raw) = accept_encoding else {
            return false;
        };
        for part in raw.split(',') {
            let token = part.trim();
            if token.is_empty() {
                continue;
            }
            let mut name = token;
            let mut q = 1.0f32;
            if let Some((n, rest)) = token.split_once(';') {
                name = n.trim();
                for param in rest.split(';') {
                    let p = param.trim();
                    if let Some(v) = p.strip_prefix("q=").or_else(|| p.strip_prefix("Q=")) {
                        if let Ok(parsed) = v.trim().parse::<f32>() {
                            q = parsed;
                        }
                    }
                }
            }
            if name.eq_ignore_ascii_case("gzip") && q > 0.0 {
                return true;
            }
        }
        false
    }

    /// Whether this content-type is eligible for gzip.
    #[must_use]
    pub fn content_type_compressible(content_type: &str) -> bool {
        let ct = content_type
            .split(';')
            .next()
            .unwrap_or(content_type)
            .trim()
            .to_ascii_lowercase();
        if ct.is_empty() {
            return false;
        }
        if ct == "text/event-stream" {
            return false;
        }
        if ct.starts_with("image/")
            || ct.starts_with("audio/")
            || ct.starts_with("video/")
            || ct == "application/gzip"
            || ct == "application/zip"
            || ct == "application/octet-stream"
        {
            return false;
        }
        ct.starts_with("text/")
            || ct == "application/json"
            || ct == "application/problem+json"
            || ct.ends_with("+json")
            || ct == "application/xml"
            || ct.ends_with("+xml")
            || ct == "application/javascript"
    }

    fn request_accept_encoding(req: &HandlerRequest) -> Option<&str> {
        req.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-encoding"))
            .map(|(_, v)| v.as_str())
    }

    fn response_content_type(res: &HandlerResponse) -> Option<&str> {
        res.get_header("content-type")
    }

    fn materialize_body(res: &HandlerResponse) -> Option<Vec<u8>> {
        // Already a raw/base64 payload — do not double-encode.
        if res
            .get_header(RAW_BODY_ENCODING_HEADER)
            .is_some_and(|v| v == RAW_BODY_ENCODING_BASE64)
        {
            return None;
        }
        match &res.body {
            Value::String(s) => Some(s.as_bytes().to_vec()),
            other => serde_json::to_vec(other).ok(),
        }
    }

    fn gzip_bytes(plain: &[u8], level: u32) -> Option<Vec<u8>> {
        let mut enc = GzEncoder::new(Vec::new(), Compression::new(level.min(9)));
        enc.write_all(plain).ok()?;
        enc.finish().ok()
    }

    fn record(&self, plain_len: usize, gz_len: usize) {
        self.compressed_responses.fetch_add(1, Ordering::Relaxed);
        self.compressed_bytes_in
            .fetch_add(plain_len as u64, Ordering::Relaxed);
        self.compressed_bytes_out
            .fetch_add(gz_len as u64, Ordering::Relaxed);
        if let Some(m) = &self.metrics {
            m.inc_compression_response();
        }
    }
}

impl Middleware for CompressionMiddleware {
    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, _latency: Duration) {
        if !self.config.enabled {
            return;
        }
        // Compress successful entity bodies only (not redirects / empty / errors).
        if !matches!(res.status, 200 | 201) {
            return;
        }
        if res.get_header("content-encoding").is_some() {
            return;
        }
        if !Self::client_accepts_gzip(Self::request_accept_encoding(req)) {
            return;
        }

        let had_content_type = Self::response_content_type(res).is_some();
        let ct = Self::response_content_type(res)
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if res.body.is_string() {
                    "text/plain".to_string()
                } else {
                    "application/json".to_string()
                }
            });
        if !Self::content_type_compressible(&ct) {
            return;
        }

        let Some(plain) = Self::materialize_body(res) else {
            return;
        };
        if plain.len() < self.config.min_bytes {
            return;
        }

        let Some(gz) = Self::gzip_bytes(&plain, self.config.level) else {
            // N3/N4: compress failure → leave identity body intact.
            return;
        };
        // Prefer identity when gzip does not shrink (CPU waste / expansion).
        if gz.len() >= plain.len() {
            return;
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(&gz);
        let vary = match res.get_header("vary") {
            Some(existing) => crate::middleware::merge_vary_field_value(
                Some(existing),
                &["Accept-Encoding"] as &[&str],
            ),
            None => "Accept-Encoding".to_string(),
        };

        res.body = Value::String(b64);
        res.set_header(
            RAW_BODY_ENCODING_HEADER,
            RAW_BODY_ENCODING_BASE64.to_string(),
        );
        res.set_header("content-encoding", "gzip".to_string());
        if !had_content_type {
            res.set_header("content-type", ct);
        }
        res.set_header("vary", vary);

        self.record(plain.len(), gz.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::HeaderVec;
    use may::sync::mpsc;
    use serde_json::json;
    use std::sync::Arc;

    fn req_ae(ae: Option<&str>) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel::<HandlerResponse>();
        let mut headers = HeaderVec::new();
        if let Some(v) = ae {
            headers.push((Arc::from("accept-encoding"), v.to_string()));
        }
        HandlerRequest {
            request_id: crate::ids::RequestId::new(),
            method: http::Method::GET,
            path: "/x".into(),
            handler_name: "h".into(),
            path_params: Default::default(),
            query_params: Default::default(),
            raw_query: None,
            headers,
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
        }
    }

    fn large_json() -> Value {
        // Well above DEFAULT_MIN_BYTES.
        let s = "x".repeat(400);
        json!({ "blob": s })
    }

    #[test]
    fn p2_no_accept_encoding_identity() {
        let mw = CompressionMiddleware::new(CompressionConfig::enabled());
        let req = req_ae(None);
        let mut res = HandlerResponse::json(200, large_json());
        mw.after(&req, &mut res, Duration::ZERO);
        assert!(res.get_header("content-encoding").is_none());
        assert!(res.get_header(RAW_BODY_ENCODING_HEADER).is_none());
    }

    #[test]
    fn p3_disabled_identity() {
        let mw = CompressionMiddleware::new(CompressionConfig::default());
        let req = req_ae(Some("gzip"));
        let mut res = HandlerResponse::json(200, large_json());
        mw.after(&req, &mut res, Duration::ZERO);
        assert!(res.get_header("content-encoding").is_none());
    }

    #[test]
    fn p1_p6_gzip_round_trip() {
        let mw = CompressionMiddleware::new(CompressionConfig::enabled());
        let req = req_ae(Some("gzip, deflate"));
        let original = large_json();
        let mut res = HandlerResponse::json(200, original.clone());
        mw.after(&req, &mut res, Duration::ZERO);
        assert_eq!(res.get_header("content-encoding"), Some("gzip"));
        assert_eq!(
            res.get_header(RAW_BODY_ENCODING_HEADER),
            Some(RAW_BODY_ENCODING_BASE64)
        );
        let Value::String(b64) = &res.body else {
            panic!("expected base64 body");
        };
        let gz = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .unwrap();
        let mut dec = flate2::read::GzDecoder::new(gz.as_slice());
        let mut plain = Vec::new();
        std::io::Read::read_to_end(&mut dec, &mut plain).unwrap();
        let got: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(got, original);
        assert_eq!(mw.compressed_responses(), 1);
    }

    #[test]
    fn p4_below_threshold_identity() {
        let mut cfg = CompressionConfig::enabled();
        cfg.min_bytes = 10_000;
        let mw = CompressionMiddleware::new(cfg);
        let req = req_ae(Some("gzip"));
        let mut res = HandlerResponse::json(200, json!({"a":1}));
        mw.after(&req, &mut res, Duration::ZERO);
        assert!(res.get_header("content-encoding").is_none());
    }

    #[test]
    fn p5_text_plain_compressible() {
        let mw = CompressionMiddleware::new(CompressionConfig::enabled());
        let req = req_ae(Some("gzip"));
        let mut headers = HeaderVec::new();
        headers.push((Arc::from("content-type"), "text/plain".into()));
        let mut res = HandlerResponse::new(200, headers, Value::String("y".repeat(400)));
        mw.after(&req, &mut res, Duration::ZERO);
        assert_eq!(res.get_header("content-encoding"), Some("gzip"));
    }

    #[test]
    fn n1_sse_never_compressed() {
        let mw = CompressionMiddleware::new(CompressionConfig::enabled());
        let req = req_ae(Some("gzip"));
        let mut headers = HeaderVec::new();
        headers.push((Arc::from("content-type"), "text/event-stream".into()));
        let mut res = HandlerResponse::new(200, headers, Value::String("data: x\n\n".repeat(80)));
        mw.after(&req, &mut res, Duration::ZERO);
        assert!(res.get_header("content-encoding").is_none());
    }

    #[test]
    fn n2_image_never_compressed() {
        assert!(!CompressionMiddleware::content_type_compressible(
            "image/png"
        ));
    }

    #[test]
    fn n5_gzip_q0_rejected() {
        assert!(!CompressionMiddleware::client_accepts_gzip(Some(
            "gzip;q=0"
        )));
        assert!(CompressionMiddleware::client_accepts_gzip(Some(
            "gzip;q=0.5"
        )));
    }

    #[test]
    fn accept_encoding_parse() {
        assert!(CompressionMiddleware::client_accepts_gzip(Some("br, gzip")));
        assert!(!CompressionMiddleware::client_accepts_gzip(Some("br")));
        assert!(!CompressionMiddleware::client_accepts_gzip(Some("")));
    }
}
