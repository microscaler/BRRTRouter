#[allow(unused_imports)]
use crate::echo::echo_handler;
use crate::router::RouteMatch;
use crate::spec::RouteMeta;
use http::Method;
use crate::ids::RequestId;
use may::coroutine;
use may::sync::mpsc;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::middleware::Middleware;

/// Generate a unique request ID for tracing (ULID string)
pub fn generate_request_id() -> String {
    RequestId::new().to_string()
}

/// Request data passed to a handler coroutine
///
/// Contains all extracted HTTP request information including path/query parameters,
/// headers, cookies, and body. Also includes a reply channel for sending the response.
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
    /// Path parameters extracted from the URL
    pub path_params: HashMap<String, String>,
    /// Query string parameters
    pub query_params: HashMap<String, String>,
    /// HTTP headers
    pub headers: HashMap<String, String>,
    /// Cookies parsed from the Cookie header
    pub cookies: HashMap<String, String>,
    /// Request body parsed as JSON (if present)
    pub body: Option<Value>,
    /// Channel for sending the response back to the dispatcher
    pub reply_tx: mpsc::Sender<HandlerResponse>,
}

/// Response data sent back from a handler coroutine
///
/// Contains the HTTP status code, headers, and JSON body to be sent to the client.
#[derive(Debug, Clone, Serialize)]
pub struct HandlerResponse {
    /// HTTP status code (200, 404, 500, etc.)
    pub status: u16,
    /// HTTP response headers
    #[serde(skip_serializing)]
    pub headers: HashMap<String, String>,
    /// Response body as JSON
    pub body: Value,
}

/// Type alias for a channel sender that dispatches requests to a handler
pub type HandlerSender = mpsc::Sender<HandlerRequest>;

/// Dispatcher that routes requests to registered handler coroutines
///
/// Maintains a registry of handler names to their corresponding channel senders,
/// and manages middleware that processes requests/responses.
#[derive(Clone, Default)]
pub struct Dispatcher {
    /// Map of handler names to their channel senders
    pub handlers: HashMap<String, HandlerSender>,
    /// Ordered list of middleware to apply to requests/responses
    pub middlewares: Vec<Arc<dyn Middleware>>,
}

impl Dispatcher {
    /// Create a new empty dispatcher
    ///
    /// Handlers must be registered using `register_handler` or `add_route`.
    pub fn new() -> Self {
        Dispatcher {
            handlers: HashMap::new(),
            middlewares: Vec::new(),
        }
    }

    #[allow(dead_code)]
    fn default() -> Self {
        Self::new()
    }

    /// Add a handler sender for the given route metadata. This allows handlers
    /// to be registered after the dispatcher has been created.
    /// 
    /// **IMPORTANT**: If a handler with the same name already exists, it will be
    /// replaced. The old sender will be dropped, which closes its channel and
    /// causes the old handler coroutine to exit when it tries to receive.
    pub fn add_route(&mut self, route: RouteMeta, sender: HandlerSender) {
        let handler_name = route.handler_name;
        
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

        let spawn_result = coroutine::Builder::new()
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
                        let error_response = HandlerResponse {
                            status: 500,
                            headers: HashMap::new(),
                            body: serde_json::json!({
                                "error": "Handler panicked",
                                "details": panic_message,
                                "request_id": request_id,
                            }),
                        };
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
            });

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

    /// Dispatch a request to the appropriate handler
    ///
    /// Sends the request to the handler's coroutine via channel and waits for the response.
    /// Returns `None` if no handler is registered for the route.
    ///
    /// # Arguments
    ///
    /// * `route_match` - Matched route with path parameters
    /// * `body` - Optional JSON request body
    /// * `headers` - HTTP headers
    /// * `cookies` - Parsed cookies
    ///
    /// # Returns
    ///
    /// * `Some(HandlerResponse)` - Response from the handler
    /// * `None` - If no handler is registered for this route
    ///
    /// # Timeout
    ///
    /// Waits up to 30 seconds for a response before timing out.
    pub fn dispatch(
        &self,
        route_match: RouteMatch,
        body: Option<Value>,
        headers: HashMap<String, String>,
        cookies: HashMap<String, String>,
    ) -> Option<HandlerResponse> {
        // Backwards-compatible wrapper: generate a request_id and call dispatch_with_request_id
        let request_id = generate_request_id();
        self.dispatch_with_request_id(route_match, body, headers, cookies, request_id)
    }

    /// Dispatch a request with a pre-determined request_id (for correlation)
    pub fn dispatch_with_request_id(
        &self,
        route_match: RouteMatch,
        body: Option<Value>,
        headers: HashMap<String, String>,
        cookies: HashMap<String, String>,
        request_id: String,
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
            path: route_match.route.path_pattern.clone(),
            handler_name: route_match.handler_name,
            path_params: route_match.path_params,
            query_params: route_match.query_params,
            headers,
            cookies,
            body,
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

            // Send request to handler coroutine
            if let Err(e) = tx.send(request.clone()) {
                error!(
                    request_id = %request_id,
                    handler_name = %request.handler_name,
                    error = %e,
                    "Failed to send request to handler"
                );
                return None;
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
                    return Some(HandlerResponse {
                        status: 503,
                        headers: HashMap::new(),
                        body: serde_json::json!({
                            "error": "Service unavailable",
                            "details": format!("Handler '{}' is not responding - possible crash or resource exhaustion", request.handler_name),
                            "request_id": request_id,
                        }),
                    });
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
}
