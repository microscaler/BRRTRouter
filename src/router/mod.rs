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
//! The router uses a radix tree (compact prefix tree) for efficient matching:
//! - O(k) matching where k is the path length (not O(n) where n is number of routes)
//! - Sub-microsecond matching for simple paths (~250-400 ns per lookup)
//! - Minimal allocations during request processing (Arc and Cow usage)
//! - Scales efficiently: 500 routes has similar performance to 10 routes
//!
//! ### Benchmark Results
//!
//! - Single route match: ~2.8 µs for 5 complex paths
//! - 10 routes: ~256 ns per match
//! - 100 routes: ~411 ns per match
//! - 500 routes: ~990 ns per match
//!
//! This represents a significant improvement over the previous O(n) linear scan
//! with regex matching, particularly for applications with many routes.

mod core;
mod radix;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod performance_tests;

pub use core::{RouteMatch, Router};
