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
/// # Safety
///
/// This function is unsafe because it spawns a coroutine that will run indefinitely
/// and handle requests. The caller must ensure that:
/// - The handler is safe to execute in a concurrent context
/// - The handler properly handles all requests without panicking
/// - The handler sends a response for every request to avoid resource leaks
/// - The May coroutine runtime is properly initialized
pub unsafe fn spawn_typed<H>(handler: H) -> mpsc::Sender<HandlerRequest>
where
    H: Handler + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<HandlerRequest>();

    may::coroutine::Builder::new()
        .stack_size(may::config().get_stack_size())
        .spawn(move || {
            let handler = handler;
            // Main event loop: process requests until channel closes
            for req in rx.iter() {
                // IMPORTANT: Clone these before entering panic-catching closure
                // We need them in the outer scope for error reporting
                let reply_tx = req.reply_tx.clone();
                let handler_name = req.handler_name.clone();

                // COMPLEX PANIC HANDLING: Wrap entire request processing in catch_unwind
                // This prevents a panicking handler from killing the entire coroutine
                // and allows us to send a 500 error response instead
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Clone reply_tx for use inside the closure (different scope)
                    let reply_tx_inner = reply_tx.clone();

                    // STEP 1: Type conversion - HandlerRequest → H::Request
                    // This validates the request data against the handler's expected type
                    let data = match H::Request::try_from(req.clone()) {
                        Ok(v) => v,
                        Err(err) => {
                            // Validation failed - send 400 Bad Request
                            let _ = reply_tx_inner.send(HandlerResponse {
                                status: 400,
                                headers: HashMap::new(),
                                body: serde_json::json!({
                                    "error": "Invalid request data",
                                    "message": err.to_string()
                                }),
                            });
                            return; // Early return from closure
                        }
                    };

                    // STEP 2: Build typed request with validated data
                    let typed_req = TypedHandlerRequest {
                        method: req.method,
                        path: req.path,
                        handler_name: req.handler_name,
                        path_params: req.path_params,
                        query_params: req.query_params,
                        data, // Strongly-typed request data
                    };

                    // STEP 3: Call the actual handler
                    let result = handler.handle(typed_req);

                    // STEP 4: Serialize and send response
                    let _ = reply_tx_inner.send(HandlerResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: serde_json::to_value(result).unwrap_or_else(
                            |_| serde_json::json!({"error": "Failed to serialize response"}),
                        ),
                    });
                }));

                // PANIC RECOVERY: If handler panicked, send 500 error
                if let Err(panic) = result {
                    let _ = reply_tx.send(HandlerResponse {
                        status: 500,
                        headers: HashMap::new(),
                        body: serde_json::json!({
                            "error": "Handler panicked",
                            "details": format!("{:?}", panic)
                        }),
                    });
                    eprintln!("Handler '{handler_name}' panicked: {panic:?}");
                }
            }
        })
        .unwrap();

    tx
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
    /// # Safety
    ///
    /// This function is unsafe because it internally calls `spawn_typed` which spawns
    /// a coroutine. The caller must ensure the same safety requirements as `spawn_typed`:
    /// - The handler is safe to execute in a concurrent context
    /// - The handler properly handles all requests without panicking
    /// - The handler sends a response for every request to avoid resource leaks
    /// - The May coroutine runtime is properly initialized
    pub unsafe fn register_typed<H>(&mut self, name: &str, handler: H)
    where
        H: Handler + Send + 'static,
    {
        let name = name.to_string();
        let tx = spawn_typed(handler);
        self.handlers.insert(name, tx);
    }
}
