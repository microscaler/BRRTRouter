//! # Server Module
//!
//! The server module provides the HTTP server implementation for BRRTRouter, built on
//! `may_minihttp` and the `may` coroutine runtime.
//!
//! ## Overview
//!
//! This module contains:
//! - [`HttpServer`] - The main HTTP server that accepts connections and routes requests
//! - [`AppService`] - The application service that integrates router, dispatcher, and middleware
//! - Request parsing and parameter extraction
//! - Response building utilities
//! - Health check endpoint
//!
//! ## Architecture
//!
//! The server follows a layered architecture:
//!
//! ```text
//! HTTP Connection → HttpServer → Middleware Chain → AppService → Router → Dispatcher → Handler
//! ```
//!
//! Each incoming request flows through:
//! 1. **HTTP Server** - Accepts connection and parses HTTP protocol
//! 2. **Middleware** - Pre-processing (auth, CORS, metrics, etc.)
//! 3. **AppService** - Coordinates routing and dispatch
//! 4. **Router** - Matches path and extracts parameters
//! 5. **Dispatcher** - Routes to appropriate handler coroutine
//! 6. **Handler** - Processes request and returns response
//!
//! ## Request Processing
//!
//! The server handles:
//! - HTTP/1.1 protocol parsing
//! - Path parameter extraction from matched routes
//! - Query string parsing
//! - Header extraction
//! - JSON body parsing
//! - Multipart form data (future)
//!
//! ## Response Building
//!
//! Responses support:
//! - JSON responses with automatic serialization
//! - Custom status codes and headers
//! - Streaming responses (Server-Sent Events)
//! - Static file serving
//!
//! ## Health Check
//!
//! The server automatically provides a `/health` endpoint that returns service status.
//!
//! ## Example
//!
//! ```rust
//! use brrtrouter::server::AppService;
//! use brrtrouter::router::Router;
//! use brrtrouter::spec::load_spec;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let spec = load_spec("openapi.yaml")?;
//! let router = Router::from_spec(&spec);
//! let service = AppService::new(router, spec);
//!
//! // Start server
//! // service.start("0.0.0.0:8080")?;
//! # Ok(())
//! # }
//! ```

/// HTTP server implementation using may_minihttp
pub mod http_server;
/// Request parsing and parameter extraction
pub mod request;
/// Response building and serialization
pub mod response;
/// Core application service that handles requests
pub mod service;

pub use request::{decode_param_value, parse_request, ParsedRequest};

pub use http_server::{HttpServer, ServerHandle};
pub use service::{health_endpoint, AppService};
