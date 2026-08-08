//! Rate limiting middleware (Epic 13.2).
//!
//! Token-bucket limiter keyed by auth subject and/or client IP. Disabled by
//! default (safe no-op). Sheds with **429** + `Retry-After` before handler
//! dispatch. Does not issue credentials — only throttles traffic.
//!
//! ## Key precedence ([`RateLimitKeyMode`])
//!
//! 1. **`SubjectThenIp`** (default): JWT `sub` when present, else client IP
//! 2. **`Subject`**: JWT `sub`, else fallback `"anonymous"`
//! 3. **`Ip`**: client IP only
//!
//! Client IP is taken from `X-Forwarded-For` (first hop), then `X-Real-IP`,
//! else `"unknown"` (no panic when peer address is absent from
//! [`HandlerRequest`](crate::dispatcher::HandlerRequest)).
//!
//! ## OPTIONS / HEAD
//!
//! All methods count toward the limit by default. Set
//! [`RateLimitConfig::skip_options`] to ignore OPTIONS (common for CORS
//! preflight). There is no silent drop — over-limit always returns 429.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde_json::Value;

use super::{MetricsMiddleware, Middleware};
use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Stable error message for shed responses (FR-5 / NFR-5).
pub const RATE_LIMIT_ERROR: &str = "rate limit exceeded";

/// How the limiter builds a bucket key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateLimitKeyMode {
    /// Prefer JWT `sub`, else client IP (documented default).
    #[default]
    SubjectThenIp,
    /// JWT `sub` only; missing subject → `"anonymous"`.
    Subject,
    /// Client IP only.
    Ip,
}

/// Configuration for [`RateLimitMiddleware`].
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// When false, middleware is a no-op (default).
    pub enabled: bool,
    /// Maximum requests allowed per window (token bucket capacity / refill target).
    pub requests: u64,
    /// Window duration for refill (tokens refill fully over this period).
    pub window: Duration,
    /// Soft cap on distinct keys; when exceeded, oldest-ish entries are evicted.
    pub max_keys: usize,
    /// Keying strategy.
    pub key_mode: RateLimitKeyMode,
    /// Per-handler tighter limits (handler_name → max requests per same window).
    pub route_limits: HashMap<String, u64>,
    /// When true, OPTIONS requests do not consume tokens.
    pub skip_options: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests: 100,
            window: Duration::from_secs(60),
            max_keys: 10_000,
            key_mode: RateLimitKeyMode::SubjectThenIp,
            route_limits: HashMap::new(),
            skip_options: false,
        }
    }
}

impl RateLimitConfig {
    /// Enabled limiter with the given global budget.
    #[must_use]
    pub fn enabled(requests: u64, window: Duration) -> Self {
        Self {
            enabled: true,
            requests: requests.max(1),
            window,
            ..Self::default()
        }
    }
}

/// Internal bucket state (token bucket).
#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// Injectable clock for deterministic tests.
pub trait RateLimitClock: Send + Sync {
    fn now(&self) -> Instant;
}

/// Wall-clock [`RateLimitClock`].
#[derive(Debug, Default)]
pub struct SystemClock;

impl RateLimitClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Test / controllable clock.
#[derive(Debug)]
pub struct ManualClock {
    inner: std::sync::Mutex<Instant>,
}

impl ManualClock {
    #[must_use]
    pub fn new(start: Instant) -> Self {
        Self {
            inner: std::sync::Mutex::new(start),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *g += by;
    }
}

impl RateLimitClock for ManualClock {
    fn now(&self) -> Instant {
        *self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Rate-limit middleware: token bucket over DashMap (no global `Mutex` per request).
pub struct RateLimitMiddleware {
    config: RateLimitConfig,
    buckets: DashMap<String, Bucket>,
    shed_total: AtomicU64,
    metrics: Option<Arc<MetricsMiddleware>>,
    clock: Arc<dyn RateLimitClock>,
}

impl RateLimitMiddleware {
    /// Create from config (system clock).
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self::with_clock(config, Arc::new(SystemClock))
    }

    /// Create with an injectable clock (tests).
    #[must_use]
    pub fn with_clock(config: RateLimitConfig, clock: Arc<dyn RateLimitClock>) -> Self {
        Self {
            config,
            buckets: DashMap::new(),
            shed_total: AtomicU64::new(0),
            metrics: None,
            clock,
        }
    }

    /// Attach metrics sink for shed counter (Prometheus via MetricsMiddleware).
    #[must_use]
    pub fn with_metrics_sink(mut self, metrics: Arc<MetricsMiddleware>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Total shed events observed by this middleware instance.
    #[must_use]
    pub fn shed_total(&self) -> u64 {
        self.shed_total.load(Ordering::Relaxed)
    }

    /// Number of live bucket keys (for eviction tests).
    #[must_use]
    pub fn key_count(&self) -> usize {
        self.buckets.len()
    }

    fn client_ip(req: &HandlerRequest) -> &str {
        if let Some(xff) = req.get_header("x-forwarded-for") {
            let first = xff.split(',').next().unwrap_or("").trim();
            if !first.is_empty() {
                return first;
            }
        }
        if let Some(real) = req.get_header("x-real-ip") {
            let t = real.trim();
            if !t.is_empty() {
                return t;
            }
        }
        "unknown"
    }

    fn subject(req: &HandlerRequest) -> Option<&str> {
        req.jwt_claims
            .as_ref()
            .and_then(|c| c.get("sub"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    fn client_key(&self, req: &HandlerRequest) -> String {
        let ip = Self::client_ip(req);
        match self.config.key_mode {
            RateLimitKeyMode::Ip => format!("ip:{ip}"),
            RateLimitKeyMode::Subject => {
                let sub = Self::subject(req).unwrap_or("anonymous");
                format!("sub:{sub}")
            }
            RateLimitKeyMode::SubjectThenIp => {
                if let Some(sub) = Self::subject(req) {
                    format!("sub:{sub}")
                } else {
                    format!("ip:{ip}")
                }
            }
        }
    }

    /// Per-client **and** per-handler so route overrides do not share a bucket
    /// with the global limit for the same client.
    fn bucket_key(&self, req: &HandlerRequest) -> String {
        format!("{}:{}", self.client_key(req), req.handler_name)
    }

    fn limit_for(&self, req: &HandlerRequest) -> u64 {
        self.config
            .route_limits
            .get(&req.handler_name)
            .copied()
            .unwrap_or(self.config.requests)
            .max(1)
    }

    fn maybe_evict_for_insert(&self) {
        let max = self.config.max_keys;
        if max == 0 {
            return;
        }
        // Evict before inserting a novel key so len stays ≤ max_keys.
        // DashMap::len is approximate under concurrency; fine for DoS mitigation.
        while self.buckets.len() >= max {
            let Some(entry) = self.buckets.iter().next().map(|e| e.key().clone()) else {
                break;
            };
            self.buckets.remove(&entry);
        }
    }

    fn try_consume(&self, key: &str, capacity: u64) -> Result<(), Duration> {
        let now = self.clock.now();
        let window = self.config.window;
        let window_secs = window.as_secs_f64().max(1e-9);
        let capacity_f = capacity as f64;

        if !self.buckets.contains_key(key) {
            self.maybe_evict_for_insert();
        }

        let mut entry = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket {
                tokens: capacity_f,
                last_refill: now,
            });

        let bucket = entry.value_mut();
        let elapsed = now.saturating_duration_since(bucket.last_refill);
        if elapsed > Duration::ZERO {
            let refill = (elapsed.as_secs_f64() / window_secs) * capacity_f;
            bucket.tokens = (bucket.tokens + refill).min(capacity_f);
            bucket.last_refill = now;
        }

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            // Seconds until one token is available.
            let need = 1.0 - bucket.tokens;
            let secs = ((need / capacity_f) * window_secs).ceil().max(1.0) as u64;
            Err(Duration::from_secs(secs))
        }
    }

    fn shed_response(retry_after: Duration) -> HandlerResponse {
        let mut resp = HandlerResponse::error(429, RATE_LIMIT_ERROR);
        resp.set_header("retry-after", retry_after.as_secs().max(1).to_string());
        resp
    }

    fn record_shed(&self) {
        self.shed_total.fetch_add(1, Ordering::Relaxed);
        if let Some(m) = &self.metrics {
            m.inc_rate_limit_shed();
        }
    }
}

impl Middleware for RateLimitMiddleware {
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        if !self.config.enabled {
            return None;
        }
        if self.config.skip_options && req.method == http::Method::OPTIONS {
            return None;
        }

        let capacity = self.limit_for(req);
        let key = self.bucket_key(req);

        match self.try_consume(&key, capacity) {
            Ok(()) => None,
            Err(retry_after) => {
                self.record_shed();
                Some(Self::shed_response(retry_after))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::HeaderVec;
    use may::sync::mpsc;

    fn req_with(handler: &str, headers: HeaderVec, claims: Option<Value>) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel::<HandlerResponse>();
        HandlerRequest {
            request_id: crate::ids::RequestId::new(),
            method: http::Method::GET,
            path: "/x".into(),
            handler_name: handler.into(),
            path_params: Default::default(),
            query_params: Default::default(),
            raw_query: None,
            headers,
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: claims,
            reply_tx: tx,
            queue_guard: None,
        }
    }

    #[test]
    fn p1_under_limit_proceeds() {
        let mw = RateLimitMiddleware::new(RateLimitConfig::enabled(5, Duration::from_secs(60)));
        let req = req_with("h", HeaderVec::new(), None);
        assert!(mw.before(&req).is_none());
    }

    #[test]
    fn p2_exactly_at_limit_then_429() {
        let mw = RateLimitMiddleware::new(RateLimitConfig::enabled(2, Duration::from_secs(60)));
        let req = req_with("h", HeaderVec::new(), None);
        assert!(mw.before(&req).is_none());
        assert!(mw.before(&req).is_none());
        let shed = mw.before(&req).expect("third must shed");
        assert_eq!(shed.status, 429);
        assert_eq!(
            shed.body.get("error").and_then(|v| v.as_str()),
            Some(RATE_LIMIT_ERROR)
        );
        assert!(shed.get_header("retry-after").is_some());
    }

    #[test]
    fn p3_window_elapses_allows_again() {
        let start = Instant::now();
        let clock = Arc::new(ManualClock::new(start));
        let mw = RateLimitMiddleware::with_clock(
            RateLimitConfig::enabled(1, Duration::from_secs(10)),
            clock.clone(),
        );
        let req = req_with("h", HeaderVec::new(), None);
        assert!(mw.before(&req).is_none());
        assert_eq!(mw.before(&req).unwrap().status, 429);
        clock.advance(Duration::from_secs(10));
        assert!(mw.before(&req).is_none());
    }

    #[test]
    fn p4_per_route_tighter_limit() {
        let mut cfg = RateLimitConfig::enabled(100, Duration::from_secs(60));
        cfg.route_limits.insert("tight".into(), 1);
        let mw = RateLimitMiddleware::new(cfg);
        let tight = req_with("tight", HeaderVec::new(), None);
        let loose = req_with("loose", HeaderVec::new(), None);
        assert!(mw.before(&tight).is_none());
        assert_eq!(mw.before(&tight).unwrap().status, 429);
        assert!(mw.before(&loose).is_none());
    }

    #[test]
    fn p5_metrics_counter_on_shed() {
        let metrics = Arc::new(MetricsMiddleware::new());
        let mw = RateLimitMiddleware::new(RateLimitConfig::enabled(1, Duration::from_secs(60)))
            .with_metrics_sink(metrics.clone());
        let req = req_with("h", HeaderVec::new(), None);
        assert!(mw.before(&req).is_none());
        assert_eq!(mw.before(&req).unwrap().status, 429);
        assert_eq!(mw.shed_total(), 1);
        assert_eq!(metrics.rate_limit_sheds(), 1);
    }

    #[test]
    fn p6_disabled_never_429() {
        let mw = RateLimitMiddleware::new(RateLimitConfig::default());
        let req = req_with("h", HeaderVec::new(), None);
        for _ in 0..50 {
            assert!(mw.before(&req).is_none());
        }
    }

    #[test]
    fn n1_burst_over_limit_no_handler_side_effect() {
        // before() short-circuits; we only assert 429 shape here.
        let mw = RateLimitMiddleware::new(RateLimitConfig::enabled(1, Duration::from_secs(60)));
        let req = req_with("h", HeaderVec::new(), None);
        let _ = mw.before(&req);
        let shed = mw.before(&req).unwrap();
        assert_eq!(shed.status, 429);
        assert!(shed.body.get("error").is_some());
    }

    #[test]
    fn n2_unique_keys_evicted() {
        let mut cfg = RateLimitConfig::enabled(1, Duration::from_secs(60));
        cfg.max_keys = 8;
        let mw = RateLimitMiddleware::new(cfg);
        for i in 0..40 {
            let mut headers = HeaderVec::new();
            headers.push((std::sync::Arc::from("x-real-ip"), format!("10.0.0.{i}")));
            let req = req_with("h", headers, None);
            let _ = mw.before(&req);
        }
        assert!(mw.key_count() <= 8, "keys={}", mw.key_count());
    }

    #[test]
    fn n3_missing_peer_uses_fallback() {
        let mw = RateLimitMiddleware::new(RateLimitConfig::enabled(2, Duration::from_secs(60)));
        let req = req_with("h", HeaderVec::new(), None);
        assert!(mw.before(&req).is_none());
        assert!(mw.before(&req).is_none());
        assert_eq!(mw.before(&req).unwrap().status, 429);
    }

    #[test]
    fn n6_skip_options_consistent() {
        let mut cfg = RateLimitConfig::enabled(1, Duration::from_secs(60));
        cfg.skip_options = true;
        let mw = RateLimitMiddleware::new(cfg);
        let (tx, _rx) = mpsc::channel::<HandlerResponse>();
        let options = HandlerRequest {
            request_id: crate::ids::RequestId::new(),
            method: http::Method::OPTIONS,
            path: "/x".into(),
            handler_name: "h".into(),
            path_params: Default::default(),
            query_params: Default::default(),
            raw_query: None,
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx: tx,
            queue_guard: None,
        };
        // OPTIONS ignored — even many of them
        for _ in 0..5 {
            assert!(mw.before(&options).is_none());
        }
        let get = req_with("h", HeaderVec::new(), None);
        assert!(mw.before(&get).is_none());
        assert_eq!(mw.before(&get).unwrap().status, 429);
    }
}
