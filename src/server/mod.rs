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
//! ## Graceful shutdown (Kubernetes)
//!
//! After [`HttpServer::start`](HttpServer::start), prefer [`ServerHandle::run_until_shutdown`]
//! instead of [`ServerHandle::join`]. On Unix the process blocks until **SIGTERM** or **SIGINT**,
//! stops the listener, then calls [`crate::otel::shutdown`] so OpenTelemetry batch exporters flush
//! before the container exits. Configure an adequate Pod **`terminationGracePeriodSeconds`** for
//! in-flight HTTP work during rollouts and scale-down.
//!
//! ## Example
//!
//! ```rust,ignore
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
pub mod app_config;
/// Inbound request-body size limits (Story 12.2)
pub mod body_limit;
pub mod cors_setup;
pub mod header_intern;
pub mod http_server;
/// Multipart/form-data body parsing (Story 12.6)
pub mod multipart;
/// Pre-handler OpenAPI parameter validation (Story 12.4)
pub mod param_validation;
/// Request parsing and parameter extraction
pub mod request;
/// Request-target boundary helpers (may_minihttp / httparse → app)
pub mod request_target;
/// Response building and serialization
pub mod response;
/// Fix B: shared service bootstrap
pub mod run_app;
/// Security provider registration from config.yaml
pub mod security_setup;
/// Core application service that handles requests
pub mod service;

pub use body_limit::{
    body_too_large_json, effective_inbound_body_limit, max_inbound_body_octets,
    DEFAULT_MAX_REQUEST_BODY_OCTETS, MAX_REQUEST_BODY_ENV, REQUEST_BODY_TOO_LARGE,
};
pub use multipart::{
    extract_boundary, multipart_error_json, multipart_error_status, parse_multipart_form_data,
    DEFAULT_MAX_FILE_PART_BYTES, MULTIPART_FILE_TOO_LARGE, MULTIPART_MALFORMED,
    MULTIPART_MISSING_BOUNDARY,
};
pub use request::{decode_param_value, parse_request, ParsedRequest};
pub use request_target::{
    assert_request_target_uri_ok, max_request_target_octets, parse_request_error_status, path_only,
    raw_query, request_target_exceeds_limit, request_target_for_app, RequestTarget,
    RequestTargetUriError, DEFAULT_MAX_REQUEST_TARGET_OCTETS, REQUEST_TARGET_TOO_LONG,
};

pub use app_config::{
    load_app_config, ApiKeyConfig, AppConfig, BearerConfig, CorsConfig, HttpConfig, JwksConfig,
    OAuth2Config, PropelAuthConfig, RemoteApiKeyConfig, SecurityConfig,
};
pub use http_server::{HttpServer, ServerHandle};
pub use run_app::{RegisterHandlersFn, RunAppArgs, RunAppBuilder, RunAppHooks};
pub use service::{health_endpoint, ready_endpoint, AppService, SharedDispatcher, SharedRouter};
