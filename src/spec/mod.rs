//! # Spec Module
//!
//! The spec module provides OpenAPI 3.1 specification parsing, loading, and processing for BRRTRouter.
//! It transforms OpenAPI documents into internal route metadata that the router and dispatcher can use.
//!
//! ## Overview
//!
//! This module is responsible for:
//! - Loading OpenAPI specifications from YAML/JSON files
//! - Parsing and validating OpenAPI 3.1 documents
//! - Building route metadata from path specifications
//! - Extracting parameter definitions and schemas
//! - Processing security requirements
//!
//! ## Key Types
//!
//! - [`RouteMeta`] - Complete metadata for a single route (path + method)
//! - [`ParameterMeta`] - Metadata for path/query/header parameters
//! - [`SecurityRequirement`] - Security scheme requirements for a route
//!
//! ## Loading Specifications
//!
//! There are three ways to load an OpenAPI spec:
//!
//! ```rust
//! use brrtrouter::spec::{load_spec, load_spec_full, load_spec_from_spec};
//! use oas3::Spec;
//!
//! // Load from file path
//! # fn example1() -> Result<(), Box<dyn std::error::Error>> {
//! let spec = load_spec("openapi.yaml")?;
//! # Ok(())
//! # }
//!
//! // Load with full route metadata extraction
//! # fn example2() -> Result<(), Box<dyn std::error::Error>> {
//! let (spec, routes) = load_spec_full("openapi.yaml")?;
//! # Ok(())
//! # }
//!
//! // Load from an already-parsed Spec object
//! # fn example3() -> Result<(), Box<dyn std::error::Error>> {
//! # let spec = Spec::default();
//! let routes = load_spec_from_spec(&spec)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Route Metadata
//!
//! For each path + method combination in the OpenAPI spec, BRRTRouter extracts:
//! - Path pattern (e.g., `/pets/{id}`)
//! - HTTP method
//! - Handler name (from `operationId` or `x-handler-name`)
//! - Path parameters with types and constraints
//! - Query parameters with types and defaults
//! - Header parameters
//! - Security requirements
//! - Request/response schemas
//!
//! ## Parameter Handling
//!
//! Parameters are extracted from the OpenAPI spec and include:
//! - Name and location (path, query, header, cookie)
//! - Type information (string, integer, etc.)
//! - Required/optional status
//! - Style (simple, form, matrix, etc.)
//! - Explode behavior for arrays/objects
//!
//! ## Example
//!
//! ```rust
//! use brrtrouter::spec::load_spec_full;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let (spec, routes) = load_spec_full("examples/openapi.yaml")?;
//!
//! for route in &routes {
//!     println!("Route: {} {}", route.method, route.path);
//!     println!("  Handler: {}", route.handler_name);
//!     println!("  Parameters: {:?}", route.params);
//! }
//! # Ok(())
//! # }
//! ```

pub use oas3::spec::{SecurityRequirement, SecurityScheme};
mod build;
mod load;
mod types;

pub use build::*;
pub use load::*;
pub use types::*;
