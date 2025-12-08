//! # Middleware Module
//!
//! The middleware module provides a composable middleware system for BRRTRouter, enabling
//! cross-cutting concerns like authentication, CORS, metrics, and tracing.
//!
//! ## Overview
//!
//! Middleware in BRRTRouter wraps the request/response cycle, allowing you to:
//! - Inspect and modify incoming requests
//! - Enforce security policies
//! - Collect metrics and traces
//! - Handle CORS preflight requests
//! - Add custom headers
//! - Short-circuit request processing (e.g., auth failures)
//!
//! ## Architecture
//!
//! Middleware uses a chain-of-responsibility pattern:
//!
//! ```text
//! Request → Middleware1 → Middleware2 → ... → Handler → ... → Middleware2 → Middleware1 → Response
//! ```
//!
//! Each middleware can:
//! - Execute code before the handler (pre-processing)
//! - Execute code after the handler (post-processing)
//! - Short-circuit the chain and return early
//!
//! ## Built-in Middleware
//!
//! - **[`AuthMiddleware`]** - Enforces authentication and authorization
//! - **[`CorsMiddleware`]** - Handles CORS headers and preflight requests
//! - **[`MetricsMiddleware`]** - Collects Prometheus metrics
//! - **[`TracingMiddleware`]** - Adds distributed tracing spans
//!
//! ## Creating Custom Middleware
//!
//! Implement the [`Middleware`] trait:
//!
//! ```rust,ignore
//! use brrtrouter::middleware::Middleware;
//! use may_minihttp::{Request, Response};
//!
//! struct CustomMiddleware;
//!
//! impl Middleware for CustomMiddleware {
//!     fn handle(&self, req: &mut Request, res: &mut Response) -> bool {
//!         // Add custom header
//!         res.headers_mut().insert("X-Custom", "value".to_string());
//!         
//!         // Return true to continue to next middleware
//!         // Return false to short-circuit
//!         true
//!     }
//! }
//! ```
//!
//! ## Middleware Ordering
//!
//! Order matters! Typical ordering:
//!
//! 1. **CORS** - Handle preflight requests early
//! 2. **Tracing** - Start spans for all requests
//! 3. **Auth** - Authenticate and authorize
//! 4. **Metrics** - Count authenticated requests
//! 5. **Handler** - Business logic
//!
//! ## Example
//!
//! ```rust,ignore
//! use brrtrouter::middleware::{AuthMiddleware, CorsMiddleware, MetricsMiddleware};
//! use brrtrouter::server::AppService;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let spec = brrtrouter::spec::load_spec("examples/openapi.yaml")?;
//! # let router = brrtrouter::router::Router::from_spec(&spec);
//! let mut service = AppService::new(router, spec);
//!
//! // Register middleware in order
//! service.add_middleware(CorsMiddleware::new());
//! service.add_middleware(AuthMiddleware::new());
//! service.add_middleware(MetricsMiddleware::new());
//! # Ok(())
//! # }
//! ```

mod auth;
mod core;
mod cors;
pub mod memory;
mod metrics;
mod tracing;

pub use auth::AuthMiddleware;
pub use core::Middleware;
pub use cors::{
    build_route_cors_map, extract_route_cors_config, CorsConfigError, CorsMiddleware,
    CorsMiddlewareBuilder, RouteCorsConfig,
};
pub use memory::MemoryMiddleware;
pub use metrics::MetricsMiddleware;
pub use tracing::TracingMiddleware;
