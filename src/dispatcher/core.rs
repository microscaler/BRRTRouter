#[allow(unused_imports)]
use crate::echo::echo_handler;
use crate::router::RouteMatch;
use crate::spec::RouteMeta;
use http::Method;
use may::coroutine;
use may::sync::mpsc;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::middleware::Middleware;

/// Request data passed to a handler coroutine
///
/// Contains all extracted HTTP request information including path/query parameters,
/// headers, cookies, and body. Also includes a reply channel for sending the response.
#[derive(Debug, Clone)]
pub struct HandlerRequest {
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
    pub fn add_route(&mut self, route: RouteMeta, sender: HandlerSender) {
        self.handlers.insert(route.handler_name, sender);
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
    /// # Safety
    ///
    /// The caller must ensure that the provided handler function is safe to execute in a
    /// concurrent context and properly handles all requests without panicking. The handler
    /// will run in a separate coroutine and must properly manage its own resources.
    /// Additionally, the handler must send a response through the reply channel for every
    /// request it receives to avoid resource leaks.
    ///
    pub unsafe fn register_handler<F>(&mut self, name: &str, handler_fn: F)
    where
        F: Fn(HandlerRequest) + Send + 'static + Clone,
    {
        let (tx, rx) = mpsc::channel::<HandlerRequest>();
        let name = name.to_string();

        coroutine::Builder::new()
            .stack_size(may::config().get_stack_size())
            .spawn(move || {
                for req in rx.iter() {
                    // Extract what we need for error handling
                    let reply_tx = req.reply_tx.clone();
                    let handler_name = req.handler_name.clone();

                    if let Err(panic) =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            handler_fn(req);
                        }))
                    {
                        // Send an error response if the handler panicked
                        let error_response = HandlerResponse {
                            status: 500,
                            headers: HashMap::new(),
                            body: serde_json::json!({
                                "error": "Handler panicked",
                                "details": format!("{:?}", panic)
                            }),
                        };
                        let _ = reply_tx.send(error_response);
                        eprintln!("Handler '{handler_name}' panicked: {panic:?}");
                    }
                }
            })
            .unwrap();

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
        let (reply_tx, reply_rx) = mpsc::channel();

        let handler_name = &route_match.handler_name;
        let tx = self.handlers.get(handler_name)?;

        let request = HandlerRequest {
            method: route_match.route.method.clone(),
            path: route_match.route.path_pattern.clone(),
            handler_name: handler_name.clone(),
            path_params: route_match.path_params.clone(),
            query_params: route_match.query_params.clone(),
            headers,
            cookies,
            body,
            reply_tx,
        };
        let mut early_resp: Option<HandlerResponse> = None;
        for mw in &self.middlewares {
            if early_resp.is_none() {
                early_resp = mw.before(&request);
            } else {
                mw.before(&request);
            }
        }
        let (mut resp, latency) = if let Some(r) = early_resp {
            (r, Duration::from_millis(0))
        } else {
            let start = Instant::now();
            tx.send(request.clone()).ok()?;
            let r = reply_rx.recv().ok()?;
            (r, start.elapsed())
        };

        for mw in &self.middlewares {
            mw.after(&request, &mut resp, latency);
        }

        Some(resp)
    }
}
