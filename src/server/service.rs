use super::request::{parse_request, ParsedRequest};
use super::response::{write_handler_response, write_json_error};
use crate::dispatcher::Dispatcher;
use crate::ids::RequestId;
use crate::middleware::MetricsMiddleware;
use crate::router::Router;
use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use crate::static_files::StaticFiles;
use crate::validator_cache::ValidatorCache;
use http::Method;
use may_minihttp::{HttpService, Request, Response};
use serde_json::json;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{info, warn};

/// HTTP application service that handles all incoming requests
///
/// This is the core service that processes HTTP requests through the full pipeline:
/// routing, authentication, validation, dispatching, and response generation.
/// It integrates all major components (router, dispatcher, middleware, security, etc.).
pub struct AppService {
    /// Router for matching requests to handlers
    pub router: Arc<RwLock<Router>>,
    /// Dispatcher for sending requests to handler coroutines
    pub dispatcher: Arc<RwLock<Dispatcher>>,
    /// Security schemes defined in the OpenAPI spec
    pub security_schemes: HashMap<String, SecurityScheme>,
    /// Active security provider implementations (API keys, JWT, OAuth2)
    pub security_providers: HashMap<String, Arc<dyn SecurityProvider>>,
    /// Optional metrics collection middleware
    pub metrics: Option<Arc<crate::middleware::MetricsMiddleware>>,
    /// Optional memory tracking middleware
    pub memory: Option<Arc<crate::middleware::MemoryMiddleware>>,
    /// Path to the OpenAPI specification file
    pub spec_path: PathBuf,
    /// Optional static file server for application files
    pub static_files: Option<StaticFiles>,
    /// Optional documentation file server (OpenAPI spec, HTML docs)
    pub doc_files: Option<StaticFiles>,
    /// Optional file watcher for hot reloading
    pub watcher: Option<notify::RecommendedWatcher>,
    /// Precomputed Keep-Alive header (to avoid per-request allocations/leaks)
    pub keep_alive_header: Option<&'static str>,
    /// JSON Schema validator cache for eliminating per-request compilation
    pub validator_cache: ValidatorCache,
}

/// Clone implementation for `AppService`
///
/// Creates a shallow clone of the service, sharing:
/// - Router (Arc-wrapped)
/// - Dispatcher (Arc-wrapped)
/// - Security schemes and providers (Arc-wrapped)
/// - Metrics middleware (Arc-wrapped)
/// - Static and doc file servers (Arc-wrapped)
///
/// **Important**: The `watcher` field is NOT cloned and is set to `None`.
/// This prevents multiple filesystem watchers from being active on clones.
/// Only the original service instance should manage hot reload.
///
/// # Use Cases
///
/// - Creating worker instances for multi-threaded servers
/// - Sharing service state across coroutines
/// - Testing with isolated service instances
impl Clone for AppService {
    /// Create a shallow clone of the service
    ///
    /// All shared state (Router, Dispatcher, etc.) is Arc-cloned (ref count bumped).
    /// The watcher is set to None to avoid duplicate filesystem watchers.
    ///
    /// # Returns
    ///
    /// A new `AppService` instance sharing the same underlying state
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            dispatcher: self.dispatcher.clone(),
            security_schemes: self.security_schemes.clone(),
            security_providers: self.security_providers.clone(),
            metrics: self.metrics.clone(),
            memory: self.memory.clone(),
            spec_path: self.spec_path.clone(),
            static_files: self.static_files.clone(),
            doc_files: self.doc_files.clone(),
            watcher: None,
            keep_alive_header: self.keep_alive_header,
            validator_cache: self.validator_cache.clone(),
        }
    }
}

impl AppService {
    /// Intern table for keep-alive header values to avoid repeated leaks
    fn intern_keep_alive(value: String) -> &'static str {
        static INTERN: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
        let map = INTERN.get_or_init(|| RwLock::new(HashMap::new()));
        // Acquire write lock to make race-free and avoid leaking duplicates.
        let mut write = map.write().expect("keep-alive interner poisoned");
        if let Some(existing) = write.get(&value).copied() {
            return existing;
        }
        let leaked: &'static str = Box::leak(value.into_boxed_str());
        write.insert(leaked.to_string(), leaked);
        leaked
    }

    /// Create a new application service
    ///
    /// # Arguments
    ///
    /// * `router` - Router for matching requests to handlers
    /// * `dispatcher` - Dispatcher for sending requests to handler coroutines
    /// * `security_schemes` - Security schemes from OpenAPI spec
    /// * `spec_path` - Path to the OpenAPI specification file
    /// * `static_dir` - Optional directory for static files
    /// * `doc_dir` - Optional directory for documentation files
    ///
    /// # Returns
    ///
    /// A new `AppService` ready to handle requests
    pub fn new(
        router: Arc<RwLock<Router>>,
        dispatcher: Arc<RwLock<Dispatcher>>,
        security_schemes: HashMap<String, SecurityScheme>,
        spec_path: PathBuf,
        static_dir: Option<PathBuf>,
        doc_dir: Option<PathBuf>,
    ) -> Self {
        // Load runtime config to determine if caching is enabled
        let runtime_config = crate::runtime_config::RuntimeConfig::from_env();
        let validator_cache = ValidatorCache::new(runtime_config.schema_cache_enabled);

        Self {
            router,
            dispatcher,
            security_schemes,
            security_providers: HashMap::new(),
            metrics: None,
            memory: None,
            spec_path,
            static_files: static_dir.map(StaticFiles::new),
            doc_files: doc_dir.map(StaticFiles::new),
            watcher: None,
            keep_alive_header: None,
            validator_cache,
        }
    }

    /// Register a security provider for authentication/authorization
    ///
    /// Security providers validate credentials (API keys, JWT tokens, OAuth2) and
    /// enforce access control based on the OpenAPI security schemes.
    ///
    /// # Arguments
    ///
    /// * `name` - Security scheme name from OpenAPI spec
    /// * `provider` - Implementation of the security provider
    pub fn register_security_provider(&mut self, name: &str, provider: Arc<dyn SecurityProvider>) {
        self.security_providers.insert(name.to_string(), provider);
    }

    /// Set the metrics collection middleware
    ///
    /// Enables Prometheus metrics collection for requests, responses, and handler performance.
    /// Automatically pre-registers all path patterns from the router to avoid runtime allocation
    /// and reduce contention during high-throughput request handling.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Metrics middleware instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let metrics = Arc::new(MetricsMiddleware::new());
    /// service.set_metrics_middleware(metrics);
    /// // All paths from OpenAPI spec are now pre-registered
    /// ```
    pub fn set_metrics_middleware(&mut self, metrics: Arc<MetricsMiddleware>) {
        // Pre-register all known paths from the router
        if let Ok(router) = self.router.read() {
            let paths = router.get_all_path_patterns();
            if !paths.is_empty() {
                info!(
                    count = paths.len(),
                    "Pre-registering paths in metrics middleware"
                );
                metrics.pre_register_paths(&paths);
            }
        }

        self.metrics = Some(metrics);
    }

    /// Set the memory tracking middleware
    ///
    /// Enables memory usage tracking and export to OpenTelemetry/Prometheus.
    ///
    /// # Arguments
    ///
    /// * `memory` - Memory middleware instance
    pub fn set_memory_middleware(&mut self, memory: Arc<crate::middleware::MemoryMiddleware>) {
        self.memory = Some(memory);
    }

    /// Configure HTTP/1.1 keep-alive headers to be sent on responses.
    /// If `enable` is false, keep-alive headers are not sent.
    ///
    /// Note: may_minihttp requires header values with 'static lifetime; we therefore
    /// allocate once and leak a single header string here to avoid per-request leaks.
    pub fn set_keep_alive(&mut self, enable: bool, timeout_secs: u64, max_requests: u64) {
        if enable {
            // Build the desired header value and intern it to reuse any previously leaked instance.
            let new_value = format!("Keep-Alive: timeout={timeout_secs}, max={max_requests}");
            let interned = Self::intern_keep_alive(new_value);
            if self.keep_alive_header == Some(interned) {
                return;
            }
            self.keep_alive_header = Some(interned);
        } else {
            self.keep_alive_header = None;
        }
    }

    /// Pre-compile and cache all JSON schemas from routes at startup
    ///
    /// This method should be called immediately after creating the service to compile
    /// all request and response schemas and cache them. This eliminates the compilation
    /// overhead on the first requests and ensures all schemas are valid at startup.
    ///
    /// # Arguments
    ///
    /// * `routes` - List of route metadata from the OpenAPI spec
    ///
    /// # Returns
    ///
    /// Number of schemas successfully compiled and cached
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut service = AppService::new(router, dispatcher, schemes, spec_path, None, None);
    /// let compiled = service.precompile_schemas(&routes);
    /// println!("Pre-compiled {} schemas", compiled);
    /// ```
    pub fn precompile_schemas(&self, routes: &[crate::spec::RouteMeta]) -> usize {
        self.validator_cache.precompile_schemas(routes)
    }

    /// Register default security providers based on loaded OpenAPI security schemes.
    ///
    /// This wires ApiKey, Bearer, and OAuth2 providers using environment variables or a
    /// provided test key for development. For ApiKey schemes, the following configuration
    /// is used (in order): per-scheme env `BRRTR_API_KEY__<SCHEME_NAME>`, global env
    /// `BRRTR_API_KEY`, then `test_api_key` argument, then fallback `"test123"`.
    pub fn register_default_security_providers_from_env(&mut self, test_api_key: Option<String>) {
        use std::sync::Arc as SyncArc;

        struct ApiKeyProvider {
            key: String,
        }
        impl SecurityProvider for ApiKeyProvider {
            fn validate(
                &self,
                scheme: &SecurityScheme,
                _scopes: &[String],
                req: &SecurityRequest,
            ) -> bool {
                match scheme {
                    SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                        "header" => {
                            // Accept either the named header or Authorization: Bearer <key> for migration convenience
                            let header_ok = req
                                .get_header(&name.to_ascii_lowercase())
                                .map(|v| v == self.key)
                                .unwrap_or(false);
                            let auth_ok = req
                                .get_header("authorization")
                                .and_then(|h| h.strip_prefix("Bearer "))
                                .map(|v| v == self.key)
                                .unwrap_or(false);
                            header_ok || auth_ok
                        }
                        "query" => req.get_query(name).map(|v| v == self.key).unwrap_or(false),
                        "cookie" => req.get_cookie(name).map(|v| v == self.key).unwrap_or(false),
                        _ => false,
                    },
                    _ => false,
                }
            }
        }

        for (scheme_name, scheme) in self.security_schemes.clone() {
            match scheme {
                SecurityScheme::ApiKey { .. } => {
                    // Per-scheme env: BRRTR_API_KEY__<SCHEME_NAME>
                    let env_key_name = format!(
                        "BRRTR_API_KEY__{}",
                        scheme_name
                            .chars()
                            .map(|c| if c.is_ascii_alphanumeric() {
                                c.to_ascii_uppercase()
                            } else {
                                '_'
                            })
                            .collect::<String>()
                    );
                    let key = std::env::var(&env_key_name)
                        .ok()
                        .or_else(|| std::env::var("BRRTR_API_KEY").ok())
                        .or_else(|| test_api_key.clone())
                        .unwrap_or_else(|| "test123".to_string());
                    self.register_security_provider(
                        &scheme_name,
                        SyncArc::new(ApiKeyProvider { key }),
                    );
                }
                SecurityScheme::Http { ref scheme, .. }
                    if scheme.eq_ignore_ascii_case("bearer") =>
                {
                    // Simple development bearer provider; real validation can be plugged in by user
                    let provider = crate::security::BearerJwtProvider::new(
                        std::env::var("BRRTR_BEARER_SIGNATURE").unwrap_or_else(|_| "sig".into()),
                    );
                    self.register_security_provider(&scheme_name, SyncArc::new(provider));
                }
                SecurityScheme::OAuth2 { .. } => {
                    let provider = crate::security::OAuth2Provider::new(
                        std::env::var("BRRTR_OAUTH2_SIGNATURE").unwrap_or_else(|_| "sig".into()),
                    );
                    self.register_security_provider(&scheme_name, SyncArc::new(provider));
                }
                _ => {}
            }
        }
    }
}

/// Basic health check endpoint returning `{ "status": "ok" }`.
pub fn health_endpoint(res: &mut Response) -> io::Result<()> {
    use crate::dispatcher::HeaderVec;
    write_handler_response(
        res,
        200,
        serde_json::json!({ "status": "ok" }),
        false,
        &HeaderVec::new(),
    );
    Ok(())
}

/// Metrics endpoint returning Prometheus text format statistics.
///
/// Exposes metrics compatible with Grafana dashboards:
/// - Active requests gauge
/// - Request counts with status code labels (for error rate)
/// - Request duration histogram (for p50/p95/p99 percentiles)
/// - Worker pool metrics (queue depth, shed count)
/// - Memory usage metrics (RSS, heap, growth)
/// - Legacy per-path metrics (backward compatible)
pub fn metrics_endpoint(
    res: &mut Response,
    metrics: &MetricsMiddleware,
    memory: Option<&crate::middleware::MemoryMiddleware>,
    dispatcher: Option<&Dispatcher>,
) -> io::Result<()> {
    let (stack_size, used_stack) = metrics.stack_usage();
    let mut body = String::with_capacity(8192); // Pre-allocate for performance

    // Active requests (NEW - for Grafana "Active Requests" panel)
    body.push_str(
        "# HELP brrtrouter_active_requests Number of requests currently being processed\n",
    );
    body.push_str("# TYPE brrtrouter_active_requests gauge\n");
    body.push_str(&format!(
        "brrtrouter_active_requests {}\n",
        metrics.active_requests()
    ));

    // Requests with status code labels (NEW - for Grafana "Error Rate" panel)
    body.push_str(
        "# HELP brrtrouter_requests_total Total number of HTTP requests by path and status\n",
    );
    body.push_str("# TYPE brrtrouter_requests_total counter\n");
    let status_stats = metrics.status_stats();
    for ((path, status), count) in &status_stats {
        let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");
        body.push_str(&format!(
            "brrtrouter_requests_total{{path=\"{escaped_path}\",status=\"{status}\"}} {count}\n",
        ));
    }

    // Request duration histogram (NEW - for Grafana "Response Latency" p50/p95/p99 panel)
    body.push_str("# HELP brrtrouter_request_duration_seconds Request duration in seconds\n");
    body.push_str("# TYPE brrtrouter_request_duration_seconds histogram\n");
    let (buckets, sum_ns, count) = metrics.histogram_data();
    let bucket_boundaries = MetricsMiddleware::histogram_buckets();

    // Emit histogram buckets
    for (i, &boundary) in bucket_boundaries.iter().enumerate() {
        body.push_str(&format!(
            "brrtrouter_request_duration_seconds_bucket{{le=\"{boundary}\"}} {}\n",
            buckets[i]
        ));
    }
    // +Inf bucket (cumulative)
    body.push_str(&format!(
        "brrtrouter_request_duration_seconds_bucket{{le=\"+Inf\"}} {}\n",
        buckets[bucket_boundaries.len()]
    ));
    // Histogram sum and count
    let sum_secs = sum_ns as f64 / 1_000_000_000.0;
    body.push_str(&format!(
        "brrtrouter_request_duration_seconds_sum {sum_secs:.6}\n",
    ));
    body.push_str(&format!(
        "brrtrouter_request_duration_seconds_count {count}\n",
    ));

    // Legacy metrics (backward compatible)
    body.push_str("# HELP brrtrouter_top_level_requests_total Total number of received requests\n");
    body.push_str("# TYPE brrtrouter_top_level_requests_total counter\n");
    body.push_str(&format!(
        "brrtrouter_top_level_requests_total {}\n",
        metrics.top_level_request_count()
    ));

    body.push_str(
        "# HELP brrtrouter_auth_failures_total Total number of authentication failures\n",
    );
    body.push_str("# TYPE brrtrouter_auth_failures_total counter\n");
    body.push_str(&format!(
        "brrtrouter_auth_failures_total {}\n",
        metrics.auth_failures()
    ));

    // Connection metrics
    body.push_str("# HELP brrtrouter_connection_closes_total Total number of connection close events (client disconnects)\n");
    body.push_str("# TYPE brrtrouter_connection_closes_total counter\n");
    body.push_str(&format!(
        "brrtrouter_connection_closes_total {}\n",
        metrics.connection_closes()
    ));

    body.push_str("# HELP brrtrouter_connection_errors_total Total number of connection errors (broken pipe, reset, etc.)\n");
    body.push_str("# TYPE brrtrouter_connection_errors_total counter\n");
    body.push_str(&format!(
        "brrtrouter_connection_errors_total {}\n",
        metrics.connection_errors()
    ));

    body.push_str("# HELP brrtrouter_connection_health_ratio Ratio of successful requests to total connection events\n");
    body.push_str("# TYPE brrtrouter_connection_health_ratio gauge\n");
    body.push_str(&format!(
        "brrtrouter_connection_health_ratio {:.4}\n",
        metrics.connection_health_ratio()
    ));

    body.push_str("# HELP brrtrouter_request_latency_seconds Average request latency in seconds\n");
    body.push_str("# TYPE brrtrouter_request_latency_seconds gauge\n");
    let avg = metrics.average_latency().as_secs_f64();
    body.push_str(&format!("brrtrouter_request_latency_seconds {avg:.6}\n",));

    body.push_str("# HELP brrtrouter_coroutine_stack_bytes Configured coroutine stack size\n");
    body.push_str("# TYPE brrtrouter_coroutine_stack_bytes gauge\n");
    body.push_str(&format!("brrtrouter_coroutine_stack_bytes {stack_size}\n"));

    body.push_str("# HELP brrtrouter_coroutine_stack_used_bytes Coroutine stack bytes used\n");
    body.push_str("# TYPE brrtrouter_coroutine_stack_used_bytes gauge\n");
    body.push_str(&format!(
        "brrtrouter_coroutine_stack_used_bytes {used_stack}\n",
    ));

    // Worker pool metrics (NEW - for backpressure monitoring)
    if let Some(disp) = dispatcher {
        let worker_metrics = disp.worker_pool_metrics();
        if !worker_metrics.is_empty() {
            body.push_str("\n# Worker Pool Metrics\n");

            body.push_str("# HELP brrtrouter_worker_pool_queue_depth Current queue depth for worker pool handlers\n");
            body.push_str("# TYPE brrtrouter_worker_pool_queue_depth gauge\n");
            for (handler, (queue_depth, _, _, _)) in &worker_metrics {
                let escaped_handler = handler.replace('\\', "\\\\").replace('"', "\\\"");
                body.push_str(&format!(
                    "brrtrouter_worker_pool_queue_depth{{handler=\"{escaped_handler}\"}} {queue_depth}\n",
                ));
            }

            body.push_str("# HELP brrtrouter_worker_pool_shed_total Total requests shed due to backpressure\n");
            body.push_str("# TYPE brrtrouter_worker_pool_shed_total counter\n");
            for (handler, (_, shed_count, _, _)) in &worker_metrics {
                let escaped_handler = handler.replace('\\', "\\\\").replace('"', "\\\"");
                body.push_str(&format!(
                    "brrtrouter_worker_pool_shed_total{{handler=\"{escaped_handler}\"}} {shed_count}\n",
                ));
            }

            body.push_str("# HELP brrtrouter_worker_pool_dispatched_total Total requests dispatched to worker pool\n");
            body.push_str("# TYPE brrtrouter_worker_pool_dispatched_total counter\n");
            for (handler, (_, _, dispatched, _)) in &worker_metrics {
                let escaped_handler = handler.replace('\\', "\\\\").replace('"', "\\\"");
                body.push_str(&format!(
                    "brrtrouter_worker_pool_dispatched_total{{handler=\"{escaped_handler}\"}} {dispatched}\n",
                ));
            }

            body.push_str("# HELP brrtrouter_worker_pool_completed_total Total requests completed by worker pool\n");
            body.push_str("# TYPE brrtrouter_worker_pool_completed_total counter\n");
            for (handler, (_, _, _, completed)) in &worker_metrics {
                let escaped_handler = handler.replace('\\', "\\\\").replace('"', "\\\"");
                body.push_str(&format!(
                    "brrtrouter_worker_pool_completed_total{{handler=\"{escaped_handler}\"}} {completed}\n",
                ));
            }
        }
    }

    // Legacy per-path metrics (backward compatible)
    let path_stats = metrics.path_stats();

    body.push_str("# HELP brrtrouter_path_requests_total Total requests per path (legacy)\n");
    body.push_str("# TYPE brrtrouter_path_requests_total counter\n");
    for (path, (count, _, _, _)) in &path_stats {
        let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");
        body.push_str(&format!(
            "brrtrouter_path_requests_total{{path=\"{escaped_path}\"}} {count}\n",
        ));
    }

    body.push_str("# HELP brrtrouter_path_latency_seconds_avg Average latency per path\n");
    body.push_str("# TYPE brrtrouter_path_latency_seconds_avg gauge\n");
    for (path, (_, avg_ns, _, _)) in &path_stats {
        let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");
        let avg_secs = (*avg_ns as f64) / 1_000_000_000.0;
        body.push_str(&format!(
            "brrtrouter_path_latency_seconds_avg{{path=\"{escaped_path}\"}} {avg_secs:.6}\n",
        ));
    }

    body.push_str("# HELP brrtrouter_path_latency_seconds_min Minimum latency per path\n");
    body.push_str("# TYPE brrtrouter_path_latency_seconds_min gauge\n");
    for (path, (_, _, min_ns, _)) in &path_stats {
        let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");
        let min_secs = (*min_ns as f64) / 1_000_000_000.0;
        body.push_str(&format!(
            "brrtrouter_path_latency_seconds_min{{path=\"{escaped_path}\"}} {min_secs:.6}\n",
        ));
    }

    body.push_str("# HELP brrtrouter_path_latency_seconds_max Maximum latency per path\n");
    body.push_str("# TYPE brrtrouter_path_latency_seconds_max gauge\n");
    for (path, (_, _, _, max_ns)) in &path_stats {
        let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");
        let max_secs = (*max_ns as f64) / 1_000_000_000.0;
        body.push_str(&format!(
            "brrtrouter_path_latency_seconds_max{{path=\"{escaped_path}\"}} {max_secs:.6}\n",
        ));
    }

    // Add memory metrics if middleware is available
    if let Some(memory_mw) = memory {
        body.push_str("\n# Memory Metrics\n");
        body.push_str(&memory_mw.export_metrics());
    }

    use crate::dispatcher::HeaderVec;
    write_handler_response(
        res,
        200,
        serde_json::Value::String(body),
        false,
        &HeaderVec::new(),
    );
    Ok(())
}

/// Streams the OpenAPI specification file as `text/yaml`.
pub fn openapi_endpoint(res: &mut Response, spec_path: &Path) -> io::Result<()> {
    match std::fs::read(spec_path) {
        Ok(bytes) => {
            res.status_code(200, "OK");
            res.header("Content-Type: text/yaml");
            res.body_vec(bytes);
        }
        Err(_) => {
            write_json_error(res, 404, serde_json::json!({ "error": "Spec not found" }));
        }
    }
    Ok(())
}

/// Serves the Swagger UI `index.html` from the configured docs directory.
pub fn swagger_ui_endpoint(res: &mut Response, docs: &StaticFiles) -> io::Result<()> {
    match docs.load("index.html", Some(&json!({ "spec_url": "/openapi.yaml" }))) {
        Ok((bytes, _)) => {
            res.status_code(200, "OK");
            res.header("Content-Type: text/html");
            res.body_vec(bytes);
        }
        Err(_) => {
            write_json_error(res, 404, serde_json::json!({ "error": "Docs not found" }));
        }
    }
    Ok(())
}

/// HTTP service implementation for `AppService`
///
/// Main request processing pipeline that handles all incoming HTTP requests.
/// This is the entry point for the `may_minihttp` HTTP server.
///
/// # Request Processing Flow
///
/// 1. **Parse Request**: Extract method, path, headers, cookies, query params, body
/// 2. **Apply Keep-Alive**: Set connection persistence headers (if configured)
/// 3. **Metrics**: Increment top-level request counter
/// 4. **Infrastructure Endpoints** (short-circuit):
///    - `GET /health` → Health check (200 OK)
///    - `GET /metrics` → Prometheus metrics
///    - `GET /openapi.yaml` → OpenAPI specification
///    - `GET /docs` → Swagger UI
/// 5. **Static Files**: Serve from `static_files` if configured (GET requests only)
/// 6. **Routing**: Match request against OpenAPI routes
/// 7. **Security Validation**: Check authentication/authorization
/// 8. **Dispatch**: Send to handler via coroutine channel
/// 9. **Response**: Write handler result to HTTP response
///
/// # Short-Circuit Paths (No Dispatch)
///
/// These endpoints bypass the dispatcher for performance:
/// - `/health` - Always returns 200 OK immediately
/// - `/metrics` - Reads atomic counters and returns Prometheus text
/// - `/openapi.yaml` - Serves spec file directly
/// - `/docs` - Renders Swagger UI template
/// - Static files - Serves from filesystem cache
///
/// # Security Enforcement
///
/// If route has `security` requirements:
/// 1. Extract credentials from request (headers/cookies)
/// 2. Call all registered `SecurityProvider` instances
/// 3. Check if ANY requirement is satisfied (OR logic)
/// 4. Return 401 if all fail, or 403 if scopes insufficient
///
/// # Error Responses
///
/// - **401 Unauthorized**: No valid credentials
/// - **403 Forbidden**: Valid credentials but insufficient scopes
/// - **404 Not Found**: No matching route
/// - **500 Internal Server Error**: Handler panic or dispatch failure
///
/// # Performance
///
/// - Infrastructure endpoints: ~50µs
/// - Static files: ~100µs (cached)
/// - Dispatched requests: ~500µs + handler time
/// - Security validation: ~50µs (simple) to ~500ms (remote)
impl HttpService for AppService {
    /// Handle an incoming HTTP request and write the response
    ///
    /// This is the main entry point called by `may_minihttp` for every request.
    /// The method is mutable to allow updating the watcher state during hot reload.
    ///
    /// # Arguments
    ///
    /// * `req` - Incoming HTTP request from `may_minihttp`
    /// * `res` - Mutable response builder to write the result
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Request processed successfully (even if response is 4xx/5xx)
    /// - `Err(io::Error)` - I/O error writing response (connection closed, etc.)
    ///
    /// # Thread Safety
    ///
    /// This method is called from multiple coroutines concurrently.
    /// All shared state (Router, Dispatcher, etc.) uses Arc + Mutex/RwLock.
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        use tracing::{debug, error, info, info_span, warn, Span};

        /// Helper struct that logs request completion when dropped
        /// This ensures we log timing even if we return early
        struct RequestLogger {
            request_id: Option<RequestId>,
            method: Method,
            path: String,
            start: std::time::Instant,
            total_size_bytes: usize,
            span: Span,
        }

        impl Drop for RequestLogger {
            fn drop(&mut self) {
                let duration_ms = self.start.elapsed().as_millis() as u64;

                // Get current coroutine stack usage if available
                // Note: May coroutines don't expose actual stack usage, only size
                let stack_used_kb = if may::coroutine::is_coroutine() {
                    let co = may::coroutine::current();
                    let size = co.stack_size();
                    (size / 1024) as u64
                } else {
                    0
                };

                // Record in span
                self.span.record("duration_ms", duration_ms);
                self.span.record("stack_used_kb", stack_used_kb);

                // R8: Request complete - Critical logging with full context
                if let Some(ref request_id) = self.request_id {
                    info!(
                        request_id = %request_id,
                        method = %self.method,
                        path = %self.path,
                        duration_ms = duration_ms,
                        stack_used_kb = stack_used_kb,
                        total_size_bytes = self.total_size_bytes,
                        "Request completed"
                    );
                } else {
                    info!(
                        method = %self.method,
                        path = %self.path,
                        duration_ms = duration_ms,
                        stack_used_kb = stack_used_kb,
                        total_size_bytes = self.total_size_bytes,
                        "Request completed"
                    );
                }
            }
        }

        // Start timing immediately
        let request_start = std::time::Instant::now();

        // Parse request and validate HTTP method
        let ParsedRequest {
            method,
            path,
            headers,
            cookies,
            query_params,
            body,
        } = match parse_request(req) {
            Ok(parsed) => parsed,
            Err(invalid_method) => {
                // Reject invalid HTTP methods with 400 Bad Request
                write_json_error(
                    res,
                    400,
                    serde_json::json!({
                        "error": "Bad Request",
                        "message": format!("Invalid HTTP method: {}", invalid_method)
                    }),
                );
                return Ok(());
            }
        };

        // Create a span for this request with key fields
        let span = info_span!(
            "http_request",
            method = %method,
            path = %path,
            header_count = headers.len(),
            status = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
            stack_used_kb = tracing::field::Empty,
        );
        let _enter = span.enter();

        // Calculate header size (always accurate)
        let header_size_bytes: usize = headers.iter().map(|(k, v)| k.len() + v.len()).sum();

        // Calculate body size using Content-Length header if available
        // This avoids expensive JSON serialization in the hot path
        let body_size_bytes = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse::<usize>().ok())
            .unwrap_or(0);

        let total_size_bytes = header_size_bytes + body_size_bytes;

        // Create request logger that will log completion on drop (RAII pattern)
        // Note: request_id will be set to None initially, updated when dispatch occurs
        // Note: total_size_bytes will be updated after routing if estimate is available
        let mut _request_logger = RequestLogger {
            request_id: None,
            method: method.clone(),
            path: path.clone(),
            start: request_start,
            total_size_bytes,
            span: span.clone(),
        };

        // Log incoming request with all headers (for debugging TooManyHeaders)
        debug!(
            method = %method,
            path = %path,
            header_count = headers.len(),
            headers = ?headers,
            query_params = ?query_params,
            cookies = ?cookies,
            body_size = body.as_ref().map(|v| v.as_object().map(|o| o.len())),
            "Request received"
        );

        // Apply keep-alive headers early so all responses inherit them
        if let Some(ka) = self.keep_alive_header {
            res.header("Connection: keep-alive");
            res.header(ka);
        }

        // Count every incoming request at top-level (even those short-circuited before dispatch)
        if let Some(metrics) = &self.metrics {
            metrics.inc_top_level_request();
        }

        if method == Method::GET && path == "/health" {
            return health_endpoint(res);
        }
        if method == Method::GET && path == "/metrics" {
            if let Some(metrics) = &self.metrics {
                // Get dispatcher for worker pool metrics (gracefully handle lock failure)
                let dispatcher_guard = self.dispatcher.read().ok();
                let dispatcher_ref = dispatcher_guard.as_deref();
                return metrics_endpoint(res, metrics, self.memory.as_deref(), dispatcher_ref);
            } else {
                write_json_error(
                    res,
                    404,
                    serde_json::json!({"error": "Not Found", "method": method.to_string(), "path": path}),
                );
                return Ok(());
            }
        }
        if method == Method::GET && path == "/openapi.yaml" {
            return openapi_endpoint(res, &self.spec_path);
        }
        if method == Method::GET && path == "/docs" {
            if let Some(docs) = &self.doc_files {
                return swagger_ui_endpoint(res, docs);
            } else {
                write_json_error(
                    res,
                    404,
                    serde_json::json!({ "error": "Docs not configured" }),
                );
                return Ok(());
            }
        }

        if method == Method::GET {
            if let Some(sf) = &self.static_files {
                let p = path.trim_start_matches('/');
                let p = if p.is_empty() { "index.html" } else { p };
                if let Ok((bytes, ct)) = sf.load(p, None) {
                    res.status_code(200, "OK");
                    let header = format!("Content-Type: {ct}").into_boxed_str();
                    res.header(Box::leak(header));
                    res.body_vec(bytes);
                    return Ok(());
                }
            }
        }

        // Determine/accept request id from headers; fallback to generated
        let inbound_req_id = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-request-id"))
            .map(|(_, v)| v.as_str())
            .filter(|s| !s.trim().is_empty());
        let canonical_req_id = RequestId::from_header_or_new(inbound_req_id);

        let route_opt = {
            // Router lock: panic on poison is appropriate - system is in undefined state
            let router = self
                .router
                .read()
                .expect("router RwLock poisoned - critical error");
            // JSF P1: method is already Method enum, no parsing needed
            // Clone method since router.route() takes ownership and we need it later for logging
            router.route(method.clone(), &path)
        };
        if let Some(mut route_match) = route_opt {
            route_match.query_params = query_params.clone();

            // Update total_size_bytes with estimated body size if Content-Length was not available
            if body_size_bytes == 0 && body.is_some() {
                if let Some(estimated) = route_match.route.estimated_request_body_bytes {
                    _request_logger.total_size_bytes = header_size_bytes + estimated;
                }
            }

            // Perform security validation first
            if !route_match.route.security.is_empty() {
                // S1: Security check start
                let schemes_required: Vec<String> = route_match
                    .route
                    .security
                    .iter()
                    .flat_map(|req| req.0.keys().cloned())
                    .collect();
                let scopes_required: Vec<String> = route_match
                    .route
                    .security
                    .iter()
                    .flat_map(|req| req.0.values().flatten().cloned())
                    .collect();

                debug!(
                    handler = %route_match.handler_name,
                    schemes_required = ?schemes_required,
                    scopes_required = ?scopes_required,
                    "Security check start"
                );

                let sec_req = SecurityRequest {
                    headers: &headers,
                    query: &query_params,
                    cookies: &cookies,
                };
                let mut authorized = false;
                let mut insufficient_scope = false;
                let mut attempted_schemes: Vec<String> = Vec::new();

                'outer: for requirement in &route_match.route.security {
                    let mut ok = true;
                    for (scheme_name, scopes) in &requirement.0 {
                        attempted_schemes.push(scheme_name.clone());

                        // S2: Security scheme lookup
                        debug!(
                            scheme_name = %scheme_name,
                            scheme_type = "lookup",
                            "Security scheme lookup"
                        );

                        let scheme = match self.security_schemes.get(scheme_name) {
                            Some(s) => s,
                            None => {
                                // S3: Provider not found
                                let available_providers: Vec<&String> =
                                    self.security_providers.keys().collect();
                                warn!(
                                    scheme_name = %scheme_name,
                                    available_providers = ?available_providers,
                                    "Security provider not found"
                                );
                                ok = false;
                                break;
                            }
                        };
                        let provider = match self.security_providers.get(scheme_name) {
                            Some(p) => p,
                            None => {
                                // S3: Provider not found (duplicate logging for consistency)
                                let available_providers: Vec<&String> =
                                    self.security_providers.keys().collect();
                                warn!(
                                    scheme_name = %scheme_name,
                                    available_providers = ?available_providers,
                                    "Security provider not found"
                                );
                                ok = false;
                                break;
                            }
                        };

                        // S4: Provider validation start
                        debug!(
                            provider_type = %scheme_name,
                            scopes = ?scopes,
                            "Provider validation start"
                        );

                        // Measure authentication/authorization performance
                        let auth_start = std::time::Instant::now();
                        let auth_result = provider.validate(scheme, scopes, &sec_req);
                        let auth_duration = auth_start.elapsed();

                        // Log slow authentication
                        if auth_duration > Duration::from_millis(100) {
                            warn!(
                                provider_type = %scheme_name,
                                duration_ms = auth_duration.as_millis(),
                                success = auth_result,
                                "Slow authentication detected"
                            );
                        } else {
                            info!(
                                provider_type = %scheme_name,
                                duration_us = auth_duration.as_micros(),
                                success = auth_result,
                                "Authentication completed"
                            );
                        }

                        if !auth_result {
                            // Detect insufficient scope for Bearer/OAuth2: token valid but scopes missing
                            match scheme {
                                SecurityScheme::Http {
                                    scheme: http_scheme,
                                    ..
                                } if http_scheme.eq_ignore_ascii_case("bearer") => {
                                    if provider.validate(scheme, &[], &sec_req) {
                                        insufficient_scope = true;
                                    }
                                }
                                SecurityScheme::OAuth2 { .. } => {
                                    if provider.validate(scheme, &[], &sec_req) {
                                        insufficient_scope = true;
                                    }
                                }
                                _ => {}
                            }
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        authorized = true;
                        break 'outer;
                    }
                }

                if !authorized {
                    if let Some(metrics) = &self.metrics {
                        metrics.inc_auth_failure();
                    }

                    let status = if insufficient_scope { 403 } else { 401 };
                    let title = if status == 403 {
                        "Forbidden"
                    } else {
                        "Unauthorized"
                    };
                    let detail = if status == 403 {
                        "Insufficient scope or permissions"
                    } else {
                        "Missing or invalid credentials"
                    };

                    // S7: Validation failed (401) or S8: Insufficient scope (403)
                    if status == 403 {
                        // S8: Insufficient scope (403)
                        warn!(
                            method = %method,
                            path = %path,
                            handler = %route_match.handler_name,
                            status = 403,
                            reason = "insufficient_scope",
                            schemes_required = ?schemes_required,
                            scopes_required = ?scopes_required,
                            attempted_schemes = ?attempted_schemes,
                            "Insufficient scope (403 forbidden)"
                        );
                    } else {
                        // S7: Validation failed (401)
                        warn!(
                            method = %method,
                            path = %path,
                            handler = %route_match.handler_name,
                            status = 401,
                            reason = "invalid_credentials",
                            schemes_required = ?schemes_required,
                            attempted_schemes = ?attempted_schemes,
                            "Authentication failed (401 unauthorized)"
                        );
                    }

                    let debug = std::env::var("BRRTR_DEBUG_VALIDATION")
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false);

                    if status == 401 {
                        res.header("WWW-Authenticate: Bearer error=\"invalid_token\"");
                    } else {
                        res.header("WWW-Authenticate: Bearer error=\"insufficient_scope\"");
                    }
                    let mut body = serde_json::json!({
                        "type": "about:blank",
                        "title": title,
                        "status": status,
                        "detail": detail
                    });
                    if debug {
                        if let Some(map) = body.as_object_mut() {
                            map.insert("method".to_string(), json!(method.to_string()));
                            map.insert("path".to_string(), json!(path));
                            map.insert(
                                "handler".to_string(),
                                // Convert Arc<str> to &str for JSON serialization
                                json!(route_match.route.handler_name.as_ref()),
                            );
                        }
                    }
                    write_json_error(res, status as u16, body);
                    return Ok(());
                } else {
                    // S6: Validation success
                    info!(
                        method = %method,
                        path = %path,
                        handler = %route_match.handler_name,
                        scheme_name = ?attempted_schemes.last(),
                        scopes_granted = ?scopes_required,
                        "Authentication success"
                    );
                }
            }

            // V2: Required body missing
            if route_match.route.request_body_required && body.is_none() {
                let expected_content_type = "application/json";
                warn!(
                    method = %method,
                    path = %path,
                    handler = %route_match.handler_name,
                    expected_content_type = %expected_content_type,
                    "Required body missing"
                );
                write_json_error(res, 400, json!({"error": "Request body required"}));
                return Ok(());
            }

            // V1 & V3: Request validation start and failure
            if let (Some(schema), Some(body_val)) = (&route_match.route.request_schema, &body) {
                // V1: Request validation start
                let schema_path = "#/components/schemas/request";
                let required_fields: Vec<String> = schema
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                debug!(
                    handler = %route_match.handler_name,
                    schema_present = true,
                    required_fields = ?required_fields,
                    "Request validation start"
                );

                // Use cached validator instead of compiling on every request
                let compiled = match self.validator_cache.get_or_compile(
                    &route_match.handler_name,
                    "request",
                    None,
                    schema,
                ) {
                    Some(v) => v,
                    None => {
                        // Schema compilation failed - this is a server configuration error
                        tracing::error!(handler = %route_match.handler_name, "Failed to compile request schema");
                        write_json_error(
                            res,
                            500,
                            serde_json::json!({
                                "error": "Internal Server Error",
                                "message": "Request schema configuration error"
                            }),
                        );
                        return Ok(());
                    }
                };
                // Use iter_errors() to get all validation errors
                let errors: Vec<_> = compiled.iter_errors(body_val).collect();
                if !errors.is_empty() {
                    // V3: Schema validation failed
                    let error_details: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                    let invalid_fields: Vec<String> = error_details
                        .iter()
                        .filter_map(|e| {
                            // Extract field names from error messages
                            e.split('\'').nth(1).map(String::from)
                        })
                        .collect();

                    warn!(
                        method = %method,
                        path = %path,
                        handler = %route_match.handler_name,
                        errors = ?error_details,
                        schema_path = %schema_path,
                        invalid_fields = ?invalid_fields,
                        "Request schema validation failed"
                    );

                    write_json_error(
                        res,
                        400,
                        json!({"error": "Request validation failed", "details": error_details}),
                    );
                    return Ok(());
                }
            }
            let is_sse = route_match.route.sse;
            // Ensure RequestLogger has the request_id for completion logs
            if _request_logger.request_id.is_none() {
                _request_logger.request_id = Some(canonical_req_id);
            }

            let handler_response = {
                let dispatcher = self
                    .dispatcher
                    .read()
                    .expect("dispatcher RwLock poisoned - critical error");
                // Determine or generate request id to pass into dispatcher
                let req_id = _request_logger
                    .request_id
                    .unwrap_or(canonical_req_id)
                    .to_string();
                dispatcher.dispatch_with_request_id(
                    route_match.clone(),
                    body,
                    headers.clone(),
                    cookies,
                    req_id,
                )
            };
            match handler_response {
                Some(hr) => {
                    let mut headers = hr.headers.clone();
                    // Always echo X-Request-ID on the response if we have one
                    // JSF P2: Use Arc::from for header names (O(1) clone, no allocation)
                    if let Some(ref rid) = _request_logger.request_id {
                        headers.push((Arc::from("x-request-id"), rid.to_string()));
                    }
                    let has_content_type = headers
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
                    if !has_content_type {
                        if let Some(ct) = route_match.route.content_type_for(hr.status) {
                            // JSF P2: Use Arc::from for header names (O(1) clone, no allocation)
                            headers.push((Arc::from("content-type"), ct));
                        }
                    }
                    if let Some(schema) = &route_match.route.response_schema {
                        // V6: Response validation start
                        debug!(
                            handler = %route_match.handler_name,
                            status = hr.status,
                            schema_present = true,
                            "Response validation start"
                        );

                        // Use cached validator instead of compiling on every response
                        // If compilation fails, skip validation but still return response
                        if let Some(compiled) = self.validator_cache.get_or_compile(
                            &route_match.handler_name,
                            "response",
                            Some(hr.status),
                            schema,
                        ) {
                            // Use iter_errors() to get all validation errors
                            let errors: Vec<_> = compiled.iter_errors(&hr.body).collect();
                            if !errors.is_empty() {
                                // V7: Response validation failed
                                let error_details: Vec<String> =
                                    errors.iter().map(|e| e.to_string()).collect();
                                let schema_path = "#/components/schemas/response";

                                error!(
                                    handler = %route_match.handler_name,
                                    status = hr.status,
                                    errors = ?error_details,
                                    schema_path = %schema_path,
                                    "Response validation failed"
                                );

                                write_json_error(
                                    res,
                                    500, // Changed from 400 to 500 since this is a server error
                                    json!({"error": "Response validation failed", "details": error_details}),
                                );
                                return Ok(());
                            }
                        } // End if let Some(compiled)
                    } // End if let Some(schema)
                    write_handler_response(res, hr.status, hr.body, is_sse, &headers);
                }
                None => {
                    write_json_error(
                        res,
                        500,
                        serde_json::json!({
                            "error": "Handler failed or not registered",
                            "method": method.to_string(),
                            "path": path
                        }),
                    );
                }
            }
        } else {
            write_json_error(
                res,
                404,
                serde_json::json!({"error": "Not Found", "method": method.to_string(), "path": path}),
            );
        }
        Ok(())
    }
}
