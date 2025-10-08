use std::time::Duration;

use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Middleware trait for intercepting requests and responses
///
/// Middleware can inspect and modify requests before they reach handlers (via `before`)
/// and responses before they're sent to clients (via `after`). Middleware is executed
/// in registration order.
///
/// # Example
///
/// ```rust,ignore
/// use brrtrouter::middleware::Middleware;
///
/// struct LoggingMiddleware;
///
/// impl Middleware for LoggingMiddleware {
///     fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
///         println!("Request: {} {}", req.method, req.path);
///         None  // Continue to handler
///     }
///
///     fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
///         println!("Response: {} in {:?}", res.status, latency);
///     }
/// }
/// ```
pub trait Middleware: Send + Sync {
    /// Called before the request is sent to the handler
    ///
    /// # Arguments
    ///
    /// * `_req` - The incoming request
    ///
    /// # Returns
    ///
    /// * `Some(HandlerResponse)` - Short-circuit and return this response immediately
    /// * `None` - Continue to the next middleware or handler
    fn before(&self, _req: &HandlerRequest) -> Option<HandlerResponse> {
        None
    }
    
    /// Called after the handler returns a response
    ///
    /// Can modify the response before it's sent to the client.
    ///
    /// # Arguments
    ///
    /// * `_req` - The original request
    /// * `_res` - The handler's response (mutable - can be modified)
    /// * `_latency` - Time taken to process the request
    fn after(&self, _req: &HandlerRequest, _res: &mut HandlerResponse, _latency: Duration) {}
}
