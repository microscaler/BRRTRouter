// typed.rs
#[allow(unused_imports)]
use crate::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse};
use anyhow::Result;
use http::Method;
use may::sync::mpsc;
use serde::Serialize;
use serde_json;
use std::collections::HashMap;
use std::convert::TryFrom;

/// Trait implemented by typed coroutine handlers.
///
/// A handler receives a [`TypedHandlerRequest`] and returns a typed response.
/// This provides type-safe request/response handling with automatic validation.
pub trait Handler: Send + 'static {
    /// The typed request type (converted from HandlerRequest)
    type Request: TryFrom<HandlerRequest, Error = anyhow::Error> + Send + 'static;
    /// The typed response type (serialized to JSON)
    type Response: Serialize + Send + 'static;

    /// Handle a typed request and return a typed response
    ///
    /// # Arguments
    ///
    /// * `req` - Typed request with validated data
    ///
    /// # Returns
    ///
    /// A typed response that will be serialized to JSON
    fn handle(&self, req: TypedHandlerRequest<Self::Request>) -> Self::Response;
}

/// Trait for converting HandlerRequest to TypedHandlerRequest
///
/// Implemented automatically for TypedHandlerRequest<T> where T can be converted from HandlerRequest.
pub trait TypedHandlerFor<T>: Sized {
    /// Convert a generic HandlerRequest to a typed request
    ///
    /// # Errors
    ///
    /// Returns an error if the request data cannot be converted to type T
    fn from_handler(req: HandlerRequest) -> anyhow::Result<TypedHandlerRequest<T>>;
}

/// Spawn a typed handler coroutine and return a sender to communicate with it.
///
/// Creates a coroutine that processes incoming requests with automatic type conversion
/// and validation. Panics in handlers are caught and converted to 500 error responses.
///
/// # Safety
///
/// This function is marked unsafe because it calls `may::coroutine::Builder::spawn()`,
/// which is unsafe in the `may` runtime. The unsafety comes from the coroutine runtime's
/// requirements, not from this function's logic.
///
/// The caller must ensure the May coroutine runtime is properly initialized.
///
/// # Handler Requirements
///
/// The handler must:
/// - Implement the `Handler` trait with typed request/response types
/// - Be safe to execute in a concurrent context
/// - Avoid long-running synchronous operations that could block the coroutine
///
/// # Panics
///
/// Handler panics are automatically caught and converted to 500 error responses.
/// The coroutine will continue processing subsequent requests.
pub unsafe fn spawn_typed<H>(handler: H) -> mpsc::Sender<HandlerRequest>
where
    H: Handler + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<HandlerRequest>();

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

    let spawn_result = may::coroutine::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            let handler = handler;
            // Main event loop: process requests until channel closes
            for req in rx.iter() {
                // Extract lightweight fields we need outside the panic-catching closure.
                // These are cheap clones (sender clones or small strings) and are ok to clone.
                let reply_tx_outer = req.reply_tx.clone();
                let handler_name_outer = req.handler_name.clone();
                let request_id = req.request_id;

                // COMPLEX PANIC HANDLING: Wrap entire request processing in catch_unwind
                // This prevents a panicking handler from killing the entire coroutine
                // and allows us to send a 500 error response instead
                //
                // KEY OPTIMIZATION: Move the owned `req` into the closure to avoid cloning it.
                // Using a move closure ensures `req` is consumed instead of cloned for each request.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
                    // Capture the outer clones into the closure scope so the closure can be moved
                    // without pulling `req` by reference.
                    let reply_tx_outer = reply_tx_outer.clone();
                    let handler = &handler; // Borrow handler so it can be reused across iterations
                    move || {
                        // Clone reply sender for inner scope use (cheap)
                        let reply_tx_inner = reply_tx_outer.clone();

                        // Extract metadata fields before consuming req in try_from
                        let method = req.method.clone();
                        let path = req.path.clone();
                        let handler_name = req.handler_name.clone();
                        let path_params = req.path_params.clone();
                        let query_params = req.query_params.clone();

                        // STEP 1: Type conversion - consume the HandlerRequest to produce handler data
                        // This intentionally consumes `req` (no req.clone()) to avoid heavy copies.
                        let data = match H::Request::try_from(req) {
                            Ok(v) => v,
                            Err(err) => {
                                // Validation failed - send 400 Bad Request
                                let _ = reply_tx_inner.send(HandlerResponse {
                                    status: 400,
                                    headers: HashMap::new(),
                                    body: serde_json::json!({
                                        "error": "Invalid request data",
                                        "message": err.to_string(),
                                        "request_id": request_id.to_string(),
                                    }),
                                });
                                return; // Early return from closure
                            }
                        };

                        // STEP 2: Build typed request with validated data
                        let typed_req = TypedHandlerRequest {
                            method,
                            path,
                            handler_name,
                            path_params,
                            query_params,
                            data, // Strongly-typed request data
                        };

                        // STEP 3: Call the actual handler
                        let result = handler.handle(typed_req);

                        // STEP 4: Serialize and send response
                        let _ = reply_tx_inner.send(HandlerResponse {
                            status: 200,
                            headers: HashMap::new(),
                            body: serde_json::to_value(result).unwrap_or_else(
                                |e| serde_json::json!({
                                    "error": "Failed to serialize response",
                                    "details": e.to_string(),
                                    "request_id": request_id.to_string(),
                                }),
                            ),
                        });
                    }
                }));

                // PANIC RECOVERY: If handler panicked, send 500 error
                if let Err(panic) = result {
                    let _ = reply_tx_outer.send(HandlerResponse {
                        status: 500,
                        headers: HashMap::new(),
                        body: serde_json::json!({
                            "error": "Handler panicked",
                            "details": format!("{:?}", panic),
                            "request_id": request_id.to_string(),
                        }),
                    });
                    eprintln!("Handler '{handler_name_outer}' panicked: {panic:?}");
                }
            }
        });

    // Handle coroutine spawn failures gracefully
    match spawn_result {
        Ok(_) => tx,
        Err(e) => {
            // Log the error and panic since we can't return an error from this function
            // In production, you might want to handle this differently
            panic!("Failed to spawn typed handler coroutine: {e}. Stack size: {stack_size} bytes. Consider increasing BRRTR_STACK_SIZE environment variable.");
        }
    }
}

/// Typed request data passed to a Handler
///
/// Contains the HTTP metadata (method, path, params) along with the typed
/// request data that has been validated and converted from the raw HandlerRequest.
#[derive(Debug, Clone)]
pub struct TypedHandlerRequest<T> {
    /// HTTP method
    pub method: Method,
    /// Request path
    pub path: String,
    /// Handler name
    pub handler_name: String,
    /// Path parameters extracted from URL
    pub path_params: HashMap<String, String>,
    /// Query string parameters
    pub query_params: HashMap<String, String>,
    /// Typed request data (validated and converted)
    pub data: T,
}

impl<T> TypedHandlerFor<T> for TypedHandlerRequest<T>
where
    T: TryFrom<HandlerRequest, Error = anyhow::Error>,
{
    fn from_handler(req: HandlerRequest) -> Result<TypedHandlerRequest<T>> {
        let data = T::try_from(req.clone())?;

        Ok(TypedHandlerRequest {
            method: req.method,
            path: req.path,
            handler_name: req.handler_name,
            path_params: req.path_params,
            query_params: req.query_params,
            data,
        })
    }
}

impl Dispatcher {
    /// Register a typed handler that converts [`HandlerRequest`] into the handler's
    /// associated request type using `TryFrom`.
    ///
    /// This spawns a coroutine that automatically validates incoming requests against
    /// the handler's expected type and converts them. Invalid requests receive a 400
    /// Bad Request response automatically.
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it internally calls `spawn_typed()` which
    /// uses `may::coroutine::Builder::spawn()`. The unsafety comes from the coroutine
    /// runtime's requirements.
    ///
    /// The caller must ensure the May coroutine runtime is properly initialized.
    ///
    /// # Handler Requirements
    ///
    /// The handler must:
    /// - Implement the `Handler` trait with typed request/response types
    /// - Be safe to execute in a concurrent context
    /// - Avoid long-running synchronous operations
    pub unsafe fn register_typed<H>(&mut self, name: &str, handler: H)
    where
        H: Handler + Send + 'static,
    {
        let name = name.to_string();
        
        // Check if we're replacing an existing handler
        if let Some(old_sender) = self.handlers.remove(&name) {
            // Drop the old sender to close its channel and stop the old coroutine
            drop(old_sender);
            eprintln!("Warning: Replacing existing typed handler '{}' - old coroutine will exit", name);
        }
        
        let tx = spawn_typed(handler);
        self.handlers.insert(name, tx);
    }

    /// Register a typed handler with a worker pool for parallel request processing
    ///
    /// This creates a pool of N worker coroutines (configured via environment variables)
    /// with a bounded queue. When the queue is full, backpressure is applied according
    /// to the configured mode (block or shed).
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it spawns coroutines. The caller must ensure
    /// the May coroutine runtime is properly initialized.
    ///
    /// # Configuration
    ///
    /// The worker pool behavior is configured via environment variables:
    /// - `BRRTR_HANDLER_WORKERS`: Number of worker coroutines (default: 4)
    /// - `BRRTR_HANDLER_QUEUE_BOUND`: Maximum queue depth (default: 1024)
    /// - `BRRTR_BACKPRESSURE_MODE`: "block" or "shed" (default: "block")
    /// - `BRRTR_BACKPRESSURE_TIMEOUT_MS`: Timeout for block mode (default: 50ms)
    pub unsafe fn register_typed_with_pool<H>(&mut self, name: &str, handler: H)
    where
        H: Handler + Send + 'static + Clone,
    {
        use crate::worker_pool::WorkerPoolConfig;
        
        let config = WorkerPoolConfig::from_env();
        
        // Create a closure that wraps the typed handler
        let handler_fn = move |req: HandlerRequest| {
            let handler = handler.clone();
            let reply_tx = req.reply_tx.clone();
            let request_id = req.request_id;
            
            // Try to convert the request
            let data = match H::Request::try_from(req.clone()) {
                Ok(v) => v,
                Err(err) => {
                    let _ = reply_tx.send(HandlerResponse {
                        status: 400,
                        headers: HashMap::new(),
                        body: serde_json::json!({
                            "error": "Invalid request data",
                            "message": err.to_string(),
                            "request_id": request_id.to_string(),
                        }),
                    });
                    return;
                }
            };
            
            // Build typed request
            let typed_req = TypedHandlerRequest {
                method: req.method.clone(),
                path: req.path.clone(),
                handler_name: req.handler_name.clone(),
                path_params: req.path_params.clone(),
                query_params: req.query_params.clone(),
                data,
            };
            
            // Call the handler
            let result = handler.handle(typed_req);
            
            // Send response
            let _ = reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
                body: serde_json::to_value(result).unwrap_or_else(|e| {
                    serde_json::json!({
                        "error": "Failed to serialize response",
                        "details": e.to_string(),
                        "request_id": request_id.to_string(),
                    })
                }),
            });
        };
        
        self.register_handler_with_pool_config(name, handler_fn, config);
    }
}
