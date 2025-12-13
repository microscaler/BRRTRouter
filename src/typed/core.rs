// typed.rs
#[allow(unused_imports)]
use crate::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
use anyhow::Result;
use http::Method;
use may::sync::mpsc;
use serde::Serialize;
use serde_json;
use std::collections::HashMap;
use std::convert::TryFrom;
use tracing::error;

/// Get the stack size for a handler with environment variable overrides applied
///
/// Checks for environment variables in this order:
/// 1. `BRRTR_STACK_SIZE__<HANDLER_NAME>` (per-handler override, uppercased)
/// 2. `BRRTR_STACK_SIZE` (global override)
/// 3. `stack_size_bytes` (computed default)
///
/// The final value is clamped to the range defined by:
/// - `BRRTR_STACK_MIN_BYTES` (default 16 KiB)
/// - `BRRTR_STACK_MAX_BYTES` (default 256 KiB)
///
/// # Arguments
///
/// * `handler_name` - Name of the handler (e.g., "list_pets")
/// * `stack_size_bytes` - Computed default stack size
///
/// # Returns
///
/// Final stack size in bytes with all overrides and clamping applied
fn get_stack_size_with_overrides(handler_name: &str, stack_size_bytes: usize) -> usize {
    // Try per-handler override first
    let env_var_name = format!("BRRTR_STACK_SIZE__{}", handler_name.to_uppercase());
    let stack_size = std::env::var(&env_var_name)
        .ok()
        .and_then(|s| {
            if let Some(hex) = s.strip_prefix("0x") {
                usize::from_str_radix(hex, 16).ok()
            } else {
                s.parse().ok()
            }
        })
        .or_else(|| {
            // Try global override
            std::env::var("BRRTR_STACK_SIZE").ok().and_then(|s| {
                if let Some(hex) = s.strip_prefix("0x") {
                    usize::from_str_radix(hex, 16).ok()
                } else {
                    s.parse().ok()
                }
            })
        })
        .unwrap_or(stack_size_bytes);

    // Apply clamping
    let min = std::env::var("BRRTR_STACK_MIN_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16 * 1024); // 16 KiB default

    let max = std::env::var("BRRTR_STACK_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256 * 1024); // 256 KiB default

    stack_size.clamp(min, max)
}

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
/// Implemented automatically for `TypedHandlerRequest<T>` where T can be converted from HandlerRequest.
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
                let _request_id = req.request_id;

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
                        // JSF: Map Arc<str> to String for HashMap
                        let path_params: HashMap<String, String> = req
                            .path_params
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect();
                        let query_params: HashMap<String, String> = req
                            .query_params
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect();

                        // STEP 1: Type conversion - consume the HandlerRequest to produce handler data
                        // This intentionally consumes `req` (no req.clone()) to avoid heavy copies.
                        let data = match H::Request::try_from(req) {
                            Ok(v) => v,
                            Err(err) => {
                                // Validation failed - send 400 Bad Request
                                let _ = reply_tx_inner.send(HandlerResponse::error(
                                    400,
                                    &format!("Invalid request data: {}", err),
                                ));
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
                        let _ = reply_tx_inner.send(HandlerResponse::json(
                            200,
                            serde_json::to_value(result).unwrap_or_else(|e| {
                                serde_json::json!({
                                    "error": "Failed to serialize response",
                                    "details": e.to_string(),
                                })
                            }),
                        ));
                    }
                }));

                // PANIC RECOVERY: If handler panicked, send 500 error
                if let Err(panic) = result {
                    let _ = reply_tx_outer.send(HandlerResponse::error(
                        500,
                        &format!("Handler panicked: {:?}", panic),
                    ));
                    eprintln!("Handler '{handler_name_outer}' panicked: {panic:?}");
                }
            }
        });

    // Handle coroutine spawn failures - this is a critical error that prevents handler operation
    // JSF Rule 115: Document panic conditions clearly
    // JSF Compliance: Panics only during initialization, never on hot path
    // This occurs during handler registration (startup), not during request handling.
    #[allow(clippy::panic)]
    match spawn_result {
        Ok(_) => tx,
        Err(e) => {
            // Log the error before panicking for better observability
            // Note: spawn_typed doesn't have handler name - use spawn_typed_with_stack_size_and_name for named handlers
            error!(
                stack_size_bytes = stack_size,
                error = %e,
                "Critical: Failed to spawn typed handler coroutine - handler will be unavailable"
            );
            // This panic is intentional: if we can't spawn the handler coroutine, the service cannot function.
            // The handler will be unavailable, so panicking during initialization is appropriate.
            panic!(
                "Failed to spawn typed handler coroutine: {}. Stack size: {} bytes. \
                Consider increasing BRRTR_STACK_SIZE environment variable.",
                e, stack_size
            );
        }
    }
}

/// Spawn a typed handler coroutine with a specific stack size and return a sender to communicate with it.
///
/// This function is similar to `spawn_typed`, but allows specifying a custom stack size
/// per handler. The stack size can be further overridden at runtime using the
/// `BRRTR_STACK_SIZE__<HANDLER_NAME>` environment variable.
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
/// # Arguments
///
/// * `handler` - The handler to spawn
/// * `stack_size_bytes` - Recommended stack size in bytes (can be overridden by environment)
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
pub unsafe fn spawn_typed_with_stack_size<H>(
    handler: H,
    stack_size_bytes: usize,
) -> mpsc::Sender<HandlerRequest>
where
    H: Handler + Send + 'static,
{
    spawn_typed_with_stack_size_and_name(handler, stack_size_bytes, None)
}

/// Spawn a typed handler coroutine with a specific stack size and handler name.
///
/// This function supports per-handler environment variable overrides via
/// `BRRTR_STACK_SIZE__<HANDLER_NAME>` when a handler name is provided.
/// The stack size can be further overridden at runtime using environment variables.
///
/// # Safety
///
/// Same safety requirements as `spawn_typed_with_stack_size`.
pub unsafe fn spawn_typed_with_stack_size_and_name<H>(
    handler: H,
    stack_size_bytes: usize,
    handler_name: Option<&str>,
) -> mpsc::Sender<HandlerRequest>
where
    H: Handler + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<HandlerRequest>();

    // Apply environment variable overrides and clamping
    // Always use get_stack_size_with_overrides to ensure consistent clamping behavior
    // When no handler name is provided, use "unknown" as a placeholder (per-handler override won't match)
    let effective_name = handler_name.unwrap_or("unknown");
    let stack_size = get_stack_size_with_overrides(effective_name, stack_size_bytes);

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
                        // Convert SmallVec to HashMap for TypedHandlerRequest API
                        // JSF: Map Arc<str> to String for HashMap
                        let path_params: HashMap<String, String> = req
                            .path_params
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect();
                        let query_params: HashMap<String, String> = req
                            .query_params
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect();

                        // STEP 1: Type conversion - consume the HandlerRequest to produce handler data
                        // This intentionally consumes `req` (no req.clone()) to avoid heavy copies.
                        let data = match H::Request::try_from(req) {
                            Ok(v) => v,
                            Err(err) => {
                                // Validation failed - send 400 Bad Request
                                let _ = reply_tx_inner.send(HandlerResponse::error(
                                    400,
                                    &format!("Invalid request data: {}", err),
                                ));
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
                            headers: HeaderVec::new(),
                            body: serde_json::to_value(result).unwrap_or_else(|e| {
                                serde_json::json!({
                                    "error": "Failed to serialize response",
                                    "details": e.to_string(),
                                    "request_id": request_id.to_string(),
                                })
                            }),
                        });
                    }
                }));

                // PANIC RECOVERY: If handler panicked, send 500 error
                if let Err(panic) = result {
                    let _ = reply_tx_outer.send(HandlerResponse {
                        status: 500,
                        headers: HeaderVec::new(),
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

    // Handle coroutine spawn failures - this is a critical error that prevents handler operation
    // JSF Rule 115: Document panic conditions clearly
    // JSF Compliance: Panics only during initialization, never on hot path
    // This occurs during handler registration (startup), not during request handling.
    #[allow(clippy::panic)]
    match spawn_result {
        Ok(_) => tx,
        Err(e) => {
            // Log the error before panicking for better observability
            error!(
                handler = %effective_name,
                stack_size_bytes = stack_size,
                error = %e,
                "Critical: Failed to spawn typed handler coroutine - handler will be unavailable"
            );
            // This panic is intentional: if we can't spawn the handler coroutine, the service cannot function.
            // The handler will be unavailable, so panicking during initialization is appropriate.
            panic!(
                "Failed to spawn typed handler coroutine for '{}': {}. Stack size: {} bytes. \
                Consider increasing BRRTR_STACK_SIZE environment variable.",
                effective_name, e, stack_size
            );
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
            // Convert SmallVec to HashMap for API compatibility
            // JSF: Map Arc<str> to String
            path_params: req
                .path_params
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            query_params: req
                .query_params
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
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
            eprintln!(
                "Warning: Replacing existing typed handler '{}' - old coroutine will exit",
                name
            );
        }

        // Also clean up any existing worker pool for this handler to prevent resource leaks
        if let Some(old_pool) = self.worker_pools.remove(&name) {
            drop(old_pool);
        }

        let tx = spawn_typed(handler);
        self.handlers.insert(name, tx);
    }

    /// Register a typed handler with a custom stack size that converts [`HandlerRequest`] into
    /// the handler's associated request type using `TryFrom`.
    ///
    /// This spawns a coroutine with the specified stack size that automatically validates
    /// incoming requests against the handler's expected type and converts them. Invalid
    /// requests receive a 400 Bad Request response automatically.
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it internally calls `spawn_typed_with_stack_size()`
    /// which uses `may::coroutine::Builder::spawn()`. The unsafety comes from the coroutine
    /// runtime's requirements.
    ///
    /// The caller must ensure the May coroutine runtime is properly initialized.
    ///
    /// # Arguments
    ///
    /// * `name` - Handler name for registration
    /// * `handler` - The handler implementation
    /// * `stack_size_bytes` - Recommended stack size in bytes
    ///
    /// # Handler Requirements
    ///
    /// The handler must:
    /// - Implement the `Handler` trait with typed request/response types
    /// - Be safe to execute in a concurrent context
    /// - Avoid long-running synchronous operations
    pub unsafe fn register_typed_with_stack_size<H>(
        &mut self,
        name: &str,
        handler: H,
        stack_size_bytes: usize,
    ) where
        H: Handler + Send + 'static,
    {
        let name = name.to_string();

        // Check if we're replacing an existing handler
        if let Some(old_sender) = self.handlers.remove(&name) {
            // Drop the old sender to close its channel and stop the old coroutine
            drop(old_sender);
            eprintln!(
                "Warning: Replacing existing typed handler '{}' - old coroutine will exit",
                name
            );
        }

        // Also clean up any existing worker pool for this handler to prevent resource leaks
        if let Some(old_pool) = self.worker_pools.remove(&name) {
            drop(old_pool);
        }

        // Use the internal function with handler name for per-handler env var support
        let tx = spawn_typed_with_stack_size_and_name(handler, stack_size_bytes, Some(&name));
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
                    let _ = reply_tx.send(HandlerResponse::error(
                        400,
                        &format!("Invalid request data: {}", err),
                    ));
                    return;
                }
            };

            // Build typed request (convert SmallVec to HashMap for API compatibility)
            // JSF: Map Arc<str> to String
            let typed_req = TypedHandlerRequest {
                method: req.method.clone(),
                path: req.path.clone(),
                handler_name: req.handler_name.clone(),
                path_params: req
                    .path_params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
                query_params: req
                    .query_params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
                data,
            };

            // Call the handler
            let result = handler.handle(typed_req);

            // Send response
            let _ = reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // These tests manipulate process-global environment variables, so they must be serialized.
    // Use a mutex to ensure only one env-var-manipulating test runs at a time.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper to clean all stack size env vars
    fn clean_stack_env_vars(handler_name: &str) {
        let env_var_name = format!("BRRTR_STACK_SIZE__{}", handler_name.to_uppercase());
        std::env::remove_var(&env_var_name);
        std::env::remove_var("BRRTR_STACK_SIZE");
        std::env::remove_var("BRRTR_STACK_MIN_BYTES");
        std::env::remove_var("BRRTR_STACK_MAX_BYTES");
    }

    #[test]
    fn test_get_stack_size_with_per_handler_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let handler = "per_handler_test";
        clean_stack_env_vars(handler);

        // Set per-handler override
        std::env::set_var("BRRTR_STACK_SIZE__PER_HANDLER_TEST", "32768");

        let stack_size = get_stack_size_with_overrides(handler, 16384);
        assert_eq!(stack_size, 32768);

        clean_stack_env_vars(handler);
    }

    #[test]
    fn test_get_stack_size_with_global_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let handler = "global_override_test";
        clean_stack_env_vars(handler);

        // Set global override
        std::env::set_var("BRRTR_STACK_SIZE", "49152");

        let stack_size = get_stack_size_with_overrides(handler, 16384);
        assert_eq!(stack_size, 49152);

        clean_stack_env_vars(handler);
    }

    #[test]
    fn test_get_stack_size_per_handler_takes_precedence() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let handler = "precedence_test";
        clean_stack_env_vars(handler);

        // Set both overrides
        std::env::set_var("BRRTR_STACK_SIZE__PRECEDENCE_TEST", "32768");
        std::env::set_var("BRRTR_STACK_SIZE", "49152");

        let stack_size = get_stack_size_with_overrides(handler, 16384);
        // Per-handler should take precedence
        assert_eq!(stack_size, 32768);

        clean_stack_env_vars(handler);
    }

    #[test]
    fn test_get_stack_size_with_hex_format() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let handler = "hex_format_test";
        clean_stack_env_vars(handler);

        // Test hex format
        std::env::set_var("BRRTR_STACK_SIZE__HEX_FORMAT_TEST", "0x10000");

        let stack_size = get_stack_size_with_overrides(handler, 16384);
        assert_eq!(stack_size, 65536);

        clean_stack_env_vars(handler);
    }

    #[test]
    fn test_get_stack_size_clamping() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let handler = "clamping_test";
        clean_stack_env_vars(handler);

        // Set custom min/max
        std::env::set_var("BRRTR_STACK_MIN_BYTES", "32768");
        std::env::set_var("BRRTR_STACK_MAX_BYTES", "65536");

        // Test clamping to min
        std::env::set_var("BRRTR_STACK_SIZE__CLAMPING_TEST", "16384");
        let stack_size = get_stack_size_with_overrides(handler, 16384);
        assert_eq!(stack_size, 32768);

        // Test clamping to max
        std::env::set_var("BRRTR_STACK_SIZE__CLAMPING_TEST", "131072");
        let stack_size = get_stack_size_with_overrides(handler, 131072);
        assert_eq!(stack_size, 65536);

        clean_stack_env_vars(handler);
    }

    #[test]
    fn test_get_stack_size_no_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let handler = "no_override_test";
        clean_stack_env_vars(handler);

        // No overrides set, should return default
        let stack_size = get_stack_size_with_overrides(handler, 16384);
        assert_eq!(stack_size, 16384);

        clean_stack_env_vars(handler);
    }
}
