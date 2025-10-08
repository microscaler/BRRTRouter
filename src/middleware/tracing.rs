use std::time::Duration;

use tracing::info_span;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Middleware for distributed tracing using the `tracing` crate
///
/// Creates spans for each HTTP request with method, path, and handler information.
/// Automatically records request start/completion with latency metrics.
pub struct TracingMiddleware;

/// OpenTelemetry-compatible tracing middleware implementation
///
/// Integrates with the `tracing` ecosystem to provide:
/// - Request/response spans for distributed tracing
/// - Structured logging with metadata (method, path, status, latency)
/// - OpenTelemetry OTLP export compatibility
/// - Jaeger/Zipkin trace visualization
///
/// # Spans Created
///
/// 1. **`http_request`** span (in `before()`):
///    - `method`: HTTP method (GET, POST, etc.)
///    - `path`: Request path
///    - `handler`: Handler function name
///
/// 2. **`http_response`** span (in `after()`):
///    - All fields from request span
///    - `status`: HTTP status code
///    - `latency_ms`: Request duration in milliseconds
///
/// # Usage
///
/// ```rust
/// use brrtrouter::middleware::TracingMiddleware;
/// use brrtrouter::dispatcher::Dispatcher;
///
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.add_middleware(std::sync::Arc::new(TracingMiddleware));
/// ```
///
/// # Integration
///
/// Configure tracing subscriber in `main.rs`:
///
/// ```rust
/// use tracing_subscriber::layer::SubscriberExt;
/// use tracing_subscriber::util::SubscriberInitExt;
///
/// tracing_subscriber::registry()
///     .with(tracing_subscriber::fmt::layer())
///     .with(opentelemetry::trace::TracerProvider::default())
///     .init();
/// ```
impl Middleware for TracingMiddleware {
    /// Create a span and log request start
    ///
    /// Captures method, path, and handler name for distributed tracing.
    /// The span is created but the guard is dropped immediately - the actual
    /// request processing happens outside this span.
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming request
    ///
    /// # Returns
    ///
    /// Always returns `None` (never blocks requests)
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        // Create and immediately record a span for this request
        let span = info_span!(
            "http_request",
            method = ?req.method,
            path = %req.path,
            handler = %req.handler_name
        );

        // Use the span to record the start event
        let _guard = span.enter();
        tracing::info!("Request started");

        None
    }

    /// Create response span and log completion with metrics
    ///
    /// Records the completed request with status code and latency.
    /// This data is exported to configured tracing backends (Jaeger, OTLP, etc.).
    ///
    /// # Arguments
    ///
    /// * `req` - The original request (for context)
    /// * `res` - The response (status code captured)
    /// * `latency` - Request processing duration
    ///
    /// # Span Lifecycle
    ///
    /// 1. Create `http_response` span with all metadata
    /// 2. Enter span context
    /// 3. Log completion event with status and latency
    /// 4. Explicitly drop guard to finalize span
    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        // Create a completed span for the response
        let span = info_span!(
            "http_response",
            method = ?req.method,
            path = %req.path,
            handler = %req.handler_name,
            status = res.status,
            latency_ms = latency.as_millis() as u64
        );

        // Use the span to record the completion event
        let _guard = span.enter();
        tracing::info!(
            status = res.status,
            latency_ms = latency.as_millis() as u64,
            "Request completed"
        );

        // Explicitly drop the guard to finish the span
        drop(_guard);
    }
}
