//! # Dispatcher Module
//!
//! The dispatcher module provides coroutine-based request handler dispatch for BRRTRouter.
//! It manages the lifecycle of handler coroutines and routes requests to the appropriate
//! handlers based on route matches from the router.
//!
//! ## Overview
//!
//! The dispatcher is the heart of BRRTRouter's concurrent request handling. It:
//! - Manages a registry of handler coroutines
//! - Routes matched requests to their corresponding handlers via channels
//! - Handles handler responses and errors
//! - Provides panic recovery for handler failures
//!
//! ## Architecture
//!
//! BRRTRouter uses the `may` coroutine runtime for efficient concurrency:
//!
//! - Each handler runs in its own coroutine (lightweight thread)
//! - Requests are sent to handlers via MPSC channels
//! - Handlers process requests and send responses back via one-shot channels
//! - Stack size is configurable via `BRRTR_STACK_SIZE` environment variable
//!
//! ## Handler Registration
//!
//! Handlers are registered with the dispatcher at startup:
//!
//! ```rust,ignore
//! use brrtrouter::dispatcher::Dispatcher;
//! use brrtrouter::server::{HandlerRequest, HandlerResponse};
//!
//! let mut dispatcher = Dispatcher::new();
//!
//! // Register a handler coroutine
//! dispatcher.register("get_pet", |req: HandlerRequest| {
//!     // Process request
//!     HandlerResponse::ok_json(serde_json::json!({
//!         "id": req.path_params.get("id")
//!     }))
//! });
//! ```
//!
//! ## Request Flow
//!
//! 1. Router matches incoming request → route metadata
//! 2. Dispatcher looks up handler by name from route metadata
//! 3. Request is sent to handler coroutine via channel
//! 4. Handler processes request and returns response
//! 5. Response is sent back to dispatcher and returned to client
//!
//! ## Error Handling
//!
//! The dispatcher provides automatic error handling:
//! - Missing handlers return 404 responses
//! - Handler panics are caught and return 500 responses
//! - Channel errors are logged and handled gracefully
//!
//! ## Performance Considerations
//!
//! - Coroutines are pre-spawned at startup (no per-request overhead)
//! - Channel operations are lock-free and very fast
//! - Stack size should be tuned based on handler complexity
//! - Default stack size is 1MB per coroutine

mod core;

pub use core::{
    generate_request_id, Dispatcher, HandlerRequest, HandlerResponse, HandlerSender, HeaderVec,
    MAX_INLINE_HEADERS,
};
