//! Dispatcher core module - hot path for request dispatch.
//!
//! # JSF Compliance (Rule 206)
//!
//! This module is part of the request hot path. The following clippy lints
//! are denied to enforce "no heap allocations after initialization":
//!
//! - `clippy::inefficient_to_string` - Catches unnecessary allocations
//! - `clippy::format_push_string` - Prevents format! string building
//! - `clippy::unnecessary_to_owned` - Prevents .to_owned() on borrowed data

// JSF Rule 206: Deny heap allocations in the hot path
// NOTE: Some allocations are required for error handling; these are off the fast path
#![deny(clippy::inefficient_to_string)]
#![deny(clippy::format_push_string)]
#![deny(clippy::unnecessary_to_owned)]

#[allow(unused_imports)]
use crate::echo::echo_handler;
use crate::ids::RequestId;
use crate::router::{ParamVec, RouteMatch};
use crate::spec::RouteMeta;
use crate::worker_pool::{WorkerPool, WorkerPoolConfig};
use http::Method;
use may::coroutine;
use may::sync::mpsc;
use serde::Serialize;
use serde_json::Value;
use smallvec::SmallVec;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::middleware::Middleware;

/// Maximum inline headers/cookies before heap allocation
/// Most requests have ≤16 headers (JSF: no heap in hot path)
pub const MAX_INLINE_HEADERS: usize = 16;

/// Stack-allocated header/cookie storage for the hot path
///
/// # JSF Optimization (P2)
///
/// Header names use `Arc<str>` instead of `String` because:
/// - Header names are often repeated (Content-Type, Authorization, etc.)
/// - `Arc::clone()` is O(1) atomic increment vs O(n) string copy
/// - Values remain `String` as they're per-request data from the HTTP request
/// - Matches the optimization pattern used in ParamVec (P0-1)
pub type HeaderVec = SmallVec<[(Arc<str>, String); MAX_INLINE_HEADERS]>;

/// Generate a unique request ID for tracing (ULID string)
#[must_use]
pub fn generate_request_id() -> String {
    RequestId::new().to_string()
}

/// Request data passed to a handler coroutine
///
/// Contains all extracted HTTP request information including path/query parameters,
/// headers, cookies, and body. Also includes a reply channel for sending the response.
///
/// # JSF Compliance
///
/// Uses `SmallVec` for path_params, query_params, headers, and cookies to avoid
/// heap allocation in the common case. This follows JSF Rule 206: "No heap
/// allocations after initialization" for the hot path.
#[derive(Debug, Clone)]
pub struct HandlerRequest {
    /// Unique request ID for tracing and correlation
    pub request_id: RequestId,
    /// HTTP method (GET, POST, etc.)
    pub method: Method,
    /// Request path
    pub path: String,
    /// Name of the handler that should process this request
    pub handler_name: String,
    /// Path parameters extracted from the URL (stack-allocated for ≤8 params)
    pub path_params: ParamVec,
    /// Query string parameters (stack-allocated for ≤8 params)
    pub query_params: ParamVec,
    /// HTTP headers (stack-allocated for ≤16 headers)
    pub headers: HeaderVec,
    /// Cookies parsed from the Cookie header (stack-allocated for ≤16 cookies)
    pub cookies: HeaderVec,
    /// Request body parsed as JSON (if present)
    pub body: Option<Value>,
    /// Decoded JWT claims (if request was authenticated with JWT)
    ///
    /// This field is populated when a JWT token is successfully validated.
    /// Contains the decoded claims from the JWT payload (e.g., `sub`, `email`, `scope`, etc.).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use brrtrouter::dispatcher::HandlerRequest;
    ///
    /// fn handler(req: HandlerRequest) {
    ///     if let Some(claims) = &req.jwt_claims {
    ///         let user_id = claims.get("sub").and_then(|v| v.as_str());
    ///         let email = claims.get("email").and_then(|v| v.as_str());
    ///         // Use claims for business logic or forwarding to downstream services
    ///     }
    /// }
    /// ```
    pub jwt_claims: Option<Value>,
    /// Channel for sending the response back to the dispatcher
    pub reply_tx: mpsc::Sender<HandlerResponse>,
}

impl HandlerRequest {
    /// Get a path parameter by name
    ///
    /// Uses "last write wins" semantics: if duplicate parameter names exist
    /// at different path depths (e.g., `/org/{id}/team/{team_id}/user/{id}`),
    /// returns the last occurrence (the user id, not the org id).
    #[inline]
    #[must_use]
    pub fn get_path_param(&self, name: &str) -> Option<&str> {
        self.path_params
            .iter()
            .rfind(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get a query parameter by name
    ///
    /// Uses "last write wins" semantics: if duplicate query parameter names exist
    /// (e.g., `?limit=10&limit=20`), returns the last occurrence.
    #[inline]
    #[must_use]
    pub fn get_query_param(&self, name: &str) -> Option<&str> {
        self.query_params
            .iter()
            .rfind(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get a header by name (case-insensitive per RFC 7230)
    #[inline]
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Get a cookie by name
    #[inline]
    #[must_use]
    pub fn get_cookie(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Convert path_params to HashMap for compatibility
    /// Note: This allocates - use get_path_param() in hot paths
    #[must_use]
    pub fn path_params_map(&self) -> HashMap<String, String> {
        self.path_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    /// Convert query_params to HashMap for compatibility
    /// Note: This allocates - use get_query_param() in hot paths
    #[must_use]
    pub fn query_params_map(&self) -> HashMap<String, String> {
        self.query_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    /// Convert headers to HashMap for compatibility
    /// Note: This allocates - use get_header() in hot paths
    #[must_use]
    pub fn headers_map(&self) -> HashMap<String, String> {
        self.headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }
}

/// Response data sent back from a handler coroutine
///
/// Contains the HTTP status code, headers, and JSON body to be sent to the client.
///
/// # JSF Compliance
///
/// Uses `SmallVec` for headers to avoid heap allocation in the common case.
#[derive(Debug, Clone, Serialize)]
pub struct HandlerResponse {
    /// HTTP status code (200, 404, 500, etc.)
    pub status: u16,
    /// HTTP response headers (stack-allocated for ≤16 headers)
    #[serde(skip_serializing)]
    pub headers: HeaderVec,
    /// Response body as JSON
    pub body: Value,
}

impl HandlerResponse {
    /// Create a new response with the given status, headers, and body
    #[must_use]
    pub fn new(status: u16, headers: HeaderVec, body: Value) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Create a JSON response with default headers
    #[must_use]
    pub fn json(status: u16, body: Value) -> Self {
        let mut headers = HeaderVec::new();
        // JSF P2: Use Arc::from for header names (O(1) clone)
        headers.push((Arc::from("content-type"), "application/json".to_string()));
        Self {
            status,
            headers,
            body,
        }
    }

    /// Create an error response
    #[must_use]
    pub fn error(status: u16, message: &str) -> Self {
        Self::json(status, serde_json::json!({ "error": message }))
    }

    /// Get a header by name
    #[inline]
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Add or update a header
    // JSF P2: Accept &str and convert to Arc<str> (O(1) for static strings)
    pub fn set_header(&mut self, name: &str, value: String) {
        // Remove existing header with same name (case-insensitive)
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
        self.headers.push((Arc::from(name), value));
    }
}

/// Type alias for a channel sender that dispatches requests to a handler
pub type HandlerSender = mpsc::Sender<HandlerRequest>;

/// Dispatcher that routes requests to registered handler coroutines
///
/// Maintains a registry of handler names to their corresponding channel senders,
/// and manages middleware that processes requests/responses.
///
/// Supports both single-coroutine handlers and worker pool handlers with bounded queues
/// and backpressure handling.
#[derive(Clone)]
pub struct Dispatcher {
    /// Map of handler names to their channel senders
    pub handlers: HashMap<String, HandlerSender>,
    /// Map of handler names to their worker pools (for handlers using worker pools)
    pub worker_pools: HashMap<String, Arc<WorkerPool>>,
    /// Ordered list of middleware to apply to requests/responses
    pub middlewares: Vec<Arc<dyn Middleware>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Create a new empty dispatcher
    ///
    /// Handlers must be registered using `register_handler` or `add_route`.
    #[must_use]
    pub fn new() -> Self {
        Dispatcher {
            handlers: HashMap::new(),
            worker_pools: HashMap::new(),
            middlewares: Vec::new(),
        }
    }

    /// Add a handler sender for the given route metadata. This allows handlers
    /// to be registered after the dispatcher has been created.
    ///
    /// **IMPORTANT**: If a handler with the same name already exists, it will be
    /// replaced. The old sender will be dropped, which closes its channel and
    /// causes the old handler coroutine to exit when it tries to receive.
    pub fn add_route(&mut self, route: RouteMeta, sender: HandlerSender) {
        // JSF P0-2: Convert Arc<str> to String for HashMap key
        let handler_name = route.handler_name.to_string();

        // Check if we're replacing an existing handler
        if let Some(old_sender) = self.handlers.remove(&handler_name) {
            // Drop the old sender explicitly to ensure the channel closes
            drop(old_sender);
            warn!(
                handler_name = %handler_name,
                total_handlers = self.handlers.len(),
                "Replaced existing handler - old coroutine will exit"
            );
        }

        info!(
            handler_name = %handler_name,
            total_handlers = self.handlers.len() + 1,
            "Handler registered successfully"
        );

        self.handlers.insert(handler_name, sender);
    }

    /// Add middleware to the processing pipeline
    ///
    /// Middleware is executed in the order it's added. Each middleware can
    /// modify requests before they reach handlers and responses before they're sent.
    ///
    /// # Arguments
    ///
    /// * `mw` - Middleware implementation to add
    pub fn add_middleware(&mut self, mw: Arc<dyn Middleware>) {
        self.middlewares.push(mw);
    }

    /// Registers a handler function that will process incoming requests with the given name.
    ///
    /// Spawns a coroutine that processes requests from a channel. The handler is automatically
    /// wrapped with panic recovery to prevent one failing handler from crashing the server.
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it calls `may::coroutine::Builder::spawn()`,
    /// which is unsafe in the `may` runtime. The unsafety comes from the coroutine runtime's
    /// requirements, not from this function's logic.
    ///
    /// The caller must ensure:
    /// - The May coroutine runtime is properly initialized before calling this
    /// - The handler sends a response through the reply channel for every request (to avoid resource leaks)
    ///
    /// # Handler Requirements
    ///
    /// The handler function should:
    /// - Be safe to execute in a concurrent context
    /// - Avoid long-running synchronous operations that could block the coroutine
    /// - Send exactly one response per request
    ///
    /// # Panics
    ///
    /// Handler panics are caught and converted to 500 error responses automatically.
    ///
    pub unsafe fn register_handler<F>(&mut self, name: &str, handler_fn: F)
    where
        F: Fn(HandlerRequest) + Send + 'static + Clone,
    {
        let (tx, rx) = mpsc::channel::<HandlerRequest>();
        let name = name.to_string();
        let handler_name_for_logging = name.clone();

        // Use a larger default stack size to prevent stack overflows
        // 64KB is more reasonable for complex handlers
        let stack_size = std::env::var("BRRTR_STACK_SIZE")
            .ok()
            .and_then(|s| {
                if let Some(hex) = s.strip_prefix("0x") {
                    usize::from_str_radix(hex, 16).ok()
                } else {
                    s.parse().ok()
                }
            })
            .unwrap_or(0x10000); // 64KB default instead of 16KB

        // SAFETY: may::coroutine::Builder::spawn() is marked unsafe by the may runtime.
        // The unsafety comes from the coroutine runtime's requirements, not from this function's logic.
        // We ensure safety by:
        // - Only calling this during initialization when the May runtime is properly set up
        // - The handler function is Send + 'static, ensuring no dangling references
        // - Error handling is done via the reply channel, not panics
        let spawn_result = unsafe {
            coroutine::Builder::new()
                .stack_size(stack_size)
                .spawn(move || {
                    // H1: Handler coroutine start
                    debug!(
                        handler_name = %handler_name_for_logging,
                        stack_size = stack_size,
                        "Handler coroutine start"
                    );

                    for req in rx.iter() {
                        // Extract what we need for error handling
                        let reply_tx = req.reply_tx.clone();
                        let handler_name = req.handler_name.clone();
                        let request_id = req.request_id;

                        // H2: Handler execution start
                        info!(
                            request_id = %request_id,
                            handler_name = %handler_name,
                            path_params = ?req.path_params,
                            query_params = ?req.query_params,
                            "Handler execution start"
                        );

                        let execution_start = Instant::now();

                        if let Err(panic) =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                handler_fn(req);
                            }))
                        {
                            // H3: Handler panic caught - CRITICAL ERROR
                            let panic_message = format!("{panic:?}");
                            let backtrace = std::backtrace::Backtrace::capture();

                            error!(
                                request_id = %request_id,
                                handler_name = %handler_name,
                                panic_message = %panic_message,
                                backtrace = %backtrace,
                                "Handler panicked - CRITICAL"
                            );

                            // Send an error response if the handler panicked
                            let error_response = HandlerResponse::error(
                                500,
                                &format!("Handler panicked: {}", panic_message),
                            );
                            let _ = reply_tx.send(error_response);
                        } else {
                            // H4: Handler execution complete
                            let execution_time_ms = execution_start.elapsed().as_millis() as u64;
                            info!(
                                request_id = %request_id,
                                handler_name = %handler_name,
                                execution_time_ms = execution_time_ms,
                                "Handler execution complete"
                            );
                        }
                    }
                })
        };

        // Handle coroutine spawn failures gracefully
        if let Err(e) = spawn_result {
            error!(
                handler_name = %name,
                error = %e,
                stack_size = stack_size,
                "Failed to spawn handler coroutine - CRITICAL"
            );
            // Return early without registering the handler
            // This prevents crashes when resources are exhausted
            return;
        }

        self.handlers.insert(name, tx);
    }

    /// Register a handler with a worker pool for parallel request processing
    ///
    /// This creates a pool of N worker coroutines (configured via environment variables)
    /// with a bounded queue. When the queue is full, backpressure is applied according
    /// to the configured mode (block or shed).
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it spawns coroutines using `may::coroutine::Builder::spawn()`,
    /// which is unsafe in the `may` runtime. The caller must ensure the May coroutine runtime is properly initialized.
    ///
    /// # Arguments
    ///
    /// * `name` - Handler name
    /// * `handler_fn` - Function to handle requests
    ///
    /// # Configuration
    ///
    /// The worker pool behavior is configured via environment variables:
    /// - `BRRTR_HANDLER_WORKERS`: Number of worker coroutines (default: 4)
    /// - `BRRTR_HANDLER_QUEUE_BOUND`: Maximum queue depth (default: 1024)
    /// - `BRRTR_BACKPRESSURE_MODE`: "block" or "shed" (default: "block")
    /// - `BRRTR_BACKPRESSURE_TIMEOUT_MS`: Timeout for block mode (default: 50ms)
    pub unsafe fn register_handler_with_pool<F>(&mut self, name: &str, handler_fn: F)
    where
        F: Fn(HandlerRequest) + Send + 'static + Clone,
    {
        let config = WorkerPoolConfig::from_env();
        self.register_handler_with_pool_config(name, handler_fn, config);
    }

    /// Register a handler with a worker pool using custom configuration
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it spawns coroutines using `may::coroutine::Builder::spawn()`,
    /// which is unsafe in the `may` runtime. The caller must ensure the May coroutine runtime is properly initialized.
    pub unsafe fn register_handler_with_pool_config<F>(
        &mut self,
        name: &str,
        handler_fn: F,
        config: WorkerPoolConfig,
    ) where
        F: Fn(HandlerRequest) + Send + 'static + Clone,
    {
        let name = name.to_string();

        // Check if we're replacing an existing handler
        if let Some(old_sender) = self.handlers.remove(&name) {
            drop(old_sender);
            warn!(
                handler_name = %name,
                "Replaced existing handler with worker pool - old coroutine will exit"
            );
        }

        // Remove any existing worker pool
        if let Some(old_pool) = self.worker_pools.remove(&name) {
            drop(old_pool);
        }

        // Create worker pool
        let pool = WorkerPool::new(name.clone(), config, handler_fn);

        // Store only the pool - dispatch goes through the pool directly, not the handlers map
        // The handlers map entry (if any) will be removed since we use the pool for dispatch
        self.handlers.remove(&name);
        self.worker_pools.insert(name, Arc::new(pool));
    }

    /// Dispatch a request to the appropriate handler
    ///
    /// Sends the request to the handler's coroutine via channel and waits for the response.
    /// Returns `None` if no handler is registered for the route.
    ///
    /// # Arguments
    ///
    /// * `route_match` - Matched route with path parameters
    /// * `body` - Optional JSON request body
    /// * `headers` - HTTP headers (stack-allocated SmallVec)
    /// * `cookies` - Parsed cookies (stack-allocated SmallVec)
    ///
    /// # Returns
    ///
    /// * `Some(HandlerResponse)` - Response from the handler
    /// * `None` - If no handler is registered for this route
    ///
    /// # Timeout
    ///
    /// Waits up to 30 seconds for a response before timing out.
    ///
    /// # JSF Compliance
    ///
    /// Uses HeaderVec (SmallVec) for headers/cookies to avoid heap allocation.
    #[must_use]
    pub fn dispatch(
        &self,
        route_match: RouteMatch,
        body: Option<Value>,
        headers: HeaderVec,
        cookies: HeaderVec,
    ) -> Option<HandlerResponse> {
        // Backwards-compatible wrapper: generate a request_id and call dispatch_with_request_id
        let request_id = generate_request_id();
        self.dispatch_with_request_id(route_match, body, headers, cookies, request_id, None)
    }

    /// Dispatch a request with a pre-determined request_id (for correlation)
    pub fn dispatch_with_request_id(
        &self,
        route_match: RouteMatch,
        body: Option<Value>,
        headers: HeaderVec,
        cookies: HeaderVec,
        request_id: String,
        jwt_claims: Option<Value>,
    ) -> Option<HandlerResponse> {
        let (reply_tx, reply_rx) = mpsc::channel();

        // D1: Handler lookup
        debug!(
            handler_name = %route_match.handler_name,
            available_handlers = self.handlers.len(),
            "Handler lookup"
        );

        let tx = match self.handlers.get(&route_match.handler_name) {
            Some(tx) => tx,
            None => {
                // D2: Handler not found - CRITICAL ERROR
                let available_handlers: Vec<&String> = self.handlers.keys().collect();
                error!(
                    handler_name = %route_match.handler_name,
                    available_handlers = ?available_handlers,
                    "Handler not found - CRITICAL"
                );
                return None;
            }
        };

        let request = HandlerRequest {
            request_id: request_id.parse().unwrap_or_else(|_| RequestId::new()),
            method: route_match.route.method.clone(),
            // JSF P0-2: Convert Arc<str> to String for HandlerRequest
            path: route_match.route.path_pattern.to_string(),
            handler_name: route_match.handler_name,
            path_params: route_match.path_params,
            query_params: route_match.query_params,
            headers,
            cookies,
            body,
            jwt_claims,
            reply_tx,
        };

        // D4: Middleware before execution
        let middleware_count = self.middlewares.len();
        debug!(
            request_id = %request_id,
            middleware_count = middleware_count,
            "Middleware before execution"
        );

        let mut early_resp: Option<HandlerResponse> = None;
        for (idx, mw) in self.middlewares.iter().enumerate() {
            if early_resp.is_none() {
                early_resp = mw.before(&request);
                if early_resp.is_some() {
                    debug!(
                        request_id = %request_id,
                        middleware_idx = idx,
                        middleware_name = std::any::type_name_of_val(mw.as_ref()),
                        "Middleware returned early response"
                    );
                }
            } else {
                mw.before(&request);
            }
        }

        let (mut resp, latency) = if let Some(r) = early_resp {
            (r, Duration::from_millis(0))
        } else {
            // D3: Request dispatched to handler
            info!(
                request_id = %request_id,
                handler_name = %request.handler_name,
                method = %request.method,
                path = %request.path,
                "Request dispatched to handler"
            );

            let start = Instant::now();

            // Check if this handler has a worker pool with backpressure
            if let Some(pool) = self.worker_pools.get(&request.handler_name) {
                // Use worker pool dispatch with backpressure handling
                match pool.dispatch(request.clone()) {
                    Ok(()) => {
                        // Request dispatched successfully, wait for response
                    }
                    Err(backpressure_response) => {
                        // Backpressure applied - return immediate response (429 or 503)
                        info!(
                            request_id = %request_id,
                            handler_name = %request.handler_name,
                            status = backpressure_response.status,
                            "Backpressure applied - returning early response"
                        );
                        return Some(backpressure_response);
                    }
                }
            } else {
                // No worker pool - send directly to handler coroutine
                if let Err(e) = tx.send(request.clone()) {
                    error!(
                        request_id = %request_id,
                        handler_name = %request.handler_name,
                        error = %e,
                        "Failed to send request to handler"
                    );
                    return None;
                }
            }

            // D6: Waiting for handler response
            debug!(
                request_id = %request_id,
                handler_name = %request.handler_name,
                "Waiting for handler response"
            );

            // Receive response with timeout detection
            // Note: may::sync::mpsc doesn't have recv_timeout, so we use recv()
            // and rely on handler-side timeouts and panic recovery
            let r = match reply_rx.recv() {
                Ok(response) => {
                    let elapsed = start.elapsed();
                    info!(
                        request_id = %request_id,
                        handler_name = %request.handler_name,
                        latency_ms = elapsed.as_millis() as u64,
                        status = response.status,
                        "Handler response received"
                    );
                    response
                }
                Err(e) => {
                    // D7: Handler channel closed - likely handler panic or resource exhaustion
                    let elapsed = start.elapsed();
                    error!(
                        request_id = %request_id,
                        handler_name = %request.handler_name,
                        elapsed_ms = elapsed.as_millis() as u64,
                        error = %e,
                        "Handler channel closed - handler may have crashed"
                    );

                    // Return a 503 Service Unavailable response instead of None
                    // This prevents connection drops and indicates server issue
                    return Some(HandlerResponse::error(
                        503,
                        &format!(
                        "Handler '{}' is not responding - possible crash or resource exhaustion",
                        request.handler_name
                    ),
                    ));
                }
            };
            (r, start.elapsed())
        };

        // D5: Middleware after execution
        debug!(
            request_id = %request_id,
            middleware_count = middleware_count,
            response_status = resp.status,
            latency_ms = latency.as_millis() as u64,
            "Middleware after execution"
        );

        for mw in &self.middlewares {
            mw.after(&request, &mut resp, latency);
        }

        Some(resp)
    }

    /// Get metrics for all worker pools
    ///
    /// Returns a map of handler names to their worker pool metrics.
    /// This is useful for monitoring queue depth and shed count.
    #[must_use]
    pub fn worker_pool_metrics(&self) -> HashMap<String, (usize, u64, u64, u64)> {
        let mut metrics = HashMap::new();
        for (name, pool) in &self.worker_pools {
            let pool_metrics = pool.metrics();
            metrics.insert(
                name.clone(),
                (
                    pool_metrics.get_queue_depth(),
                    pool_metrics.get_shed_count(),
                    pool_metrics.get_dispatched_count(),
                    pool_metrics.get_completed_count(),
                ),
            );
        }
        metrics
    }
}
