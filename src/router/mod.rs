//! # Router Module
//!
//! The router module provides path matching and route resolution functionality for BRRTRouter.
//! It uses regex-based path matching to efficiently match incoming requests to handler functions
//! defined in the OpenAPI specification.
//!
//! ## Overview
//!
//! The router is responsible for:
//! - Building routing tables from OpenAPI path specifications
//! - Matching incoming HTTP requests to registered routes
//! - Extracting path parameters from matched routes
//! - Providing route metadata for downstream processing
//!
//! ## Architecture
//!
//! The router uses a two-phase approach:
//!
//! 1. **Compilation**: At startup, OpenAPI paths (e.g., `/pets/{id}`) are converted into
//!    regex patterns that can match and extract path parameters.
//!
//! 2. **Matching**: For each incoming request, the router tests the request path against
//!    all compiled patterns until a match is found, returning route metadata and extracted
//!    parameters.
//!
//! ## Example
//!
//! ```rust,ignore
//! use brrtrouter::router::Router;
//! use brrtrouter::spec::load_spec;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load OpenAPI spec
//! let spec = load_spec("examples/openapi.yaml")?;
//!
//! // Create router from spec
//! let router = Router::from_spec(&spec);
//!
//! // Match an incoming request
//! if let Some(route_match) = router.match_route("GET", "/pets/123") {
//!     println!("Handler: {}", route_match.handler_name);
//!     println!("Path params: {:?}", route_match.path_params);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Performance
//!
//! The router uses compiled regex patterns for efficient matching, targeting:
//! - Sub-microsecond matching for simple paths
//! - Minimal allocations during request processing
//! - O(n) complexity where n is the number of routes (not request complexity)

mod core;
mod radix;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod performance_tests;

pub use core::{RouteMatch, Router};
