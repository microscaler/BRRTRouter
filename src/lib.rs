//! # BRRTRouter
//!
//! **BRRTRouter** is a high-performance, coroutine-powered HTTP router for Rust, driven entirely by
//! an [OpenAPI 3.1.0](https://spec.openapis.org/oas/v3.1.0) specification.
//!
//! ## Overview
//!
//! BRRTRouter provides a complete solution for building OpenAPI-first HTTP services in Rust using
//! the `may` coroutine runtime. It automatically generates routing tables from your OpenAPI spec,
//! handles request dispatching, parameter extraction, validation, and security enforcement.
//!
//! ## Architecture
//!
//! The library is organized into several key modules:
//!
//! - **[`spec`]** - OpenAPI 3.1 specification parsing and loading
//! - **[`router`]** - Path matching and route resolution using regex-based matchers
//! - **[`dispatcher`]** - Coroutine-based request handler dispatch
//! - **[`server`]** - HTTP server built on `may_minihttp` with request/response types
//! - **[`middleware`]** - Pluggable middleware (metrics, CORS, authentication, tracing)
//! - **[`security`]** - Security provider implementations (API keys, JWT, OAuth2)
//! - **[`generator`]** - Code generator that creates example projects from OpenAPI specs
//! - **[`typed`]** - Type-safe request/response handler traits
//! - **[`validator`]** - Request and response validation against OpenAPI schemas
//! - **[`hot_reload`]** - Live reloading of OpenAPI specifications
//! - **[`sse`]** - Server-Sent Events support
//! - **[`static_files`]** - Static file serving utilities
//!
//! ### Code Generation Flow
//!
//! The code generator transforms an OpenAPI specification into a complete, runnable service:
//!
//! ```mermaid
//! sequenceDiagram
//!     participant User
//!     participant CLI as CLI<br/>(brrtrouter-gen)
//!     participant Spec as spec::load_spec
//!     participant Build as spec::build_routes
//!     participant Schema as generator::schema
//!     participant Templates as generator::templates
//!     participant Project as generator::project
//!     participant FS as File System
//!
//!     User->>CLI: cargo run --bin brrtrouter-gen<br/>generate --spec openapi.yaml
//!     CLI->>Spec: load_spec("openapi.yaml")
//!     Spec->>Spec: Parse YAML/JSON
//!     Spec->>Build: build_routes(&spec)
//!     Build->>Build: Extract paths, methods,<br/>parameters, schemas
//!     Build-->>Spec: Vec<RouteMeta>
//!     Spec-->>CLI: (Spec, Vec<RouteMeta>)
//!     
//!     CLI->>Schema: analyze_schemas(&spec)
//!     Schema->>Schema: Walk schema definitions
//!     Schema->>Schema: Infer Rust types<br/>(String, i64, Vec<T>, etc.)
//!     Schema->>Schema: Build type dependency graph
//!     Schema-->>CLI: Vec<TypeDefinition>
//!     
//!     CLI->>Templates: render_handler_template(route)
//!     Templates->>Templates: Apply Askama template
//!     Templates-->>CLI: Generated handler code
//!     
//!     CLI->>Templates: render_controller_template(route)
//!     Templates->>Templates: Apply Askama template
//!     Templates-->>CLI: Generated controller code
//!     
//!     CLI->>Templates: render_main_template(routes)
//!     Templates-->>CLI: Generated main.rs
//!     
//!     CLI->>Templates: render_registry_template(routes)
//!     Templates-->>CLI: Generated registry.rs
//!     
//!     CLI->>Templates: render_cargo_toml(project_name)
//!     Templates-->>CLI: Generated Cargo.toml
//!     
//!     CLI->>Project: write_project_files(output_dir)
//!     Project->>FS: Create directory structure
//!     Project->>FS: Write src/main.rs
//!     Project->>FS: Write src/registry.rs
//!     Project->>FS: Write src/handlers/*.rs
//!     Project->>FS: Write src/controllers/*.rs
//!     Project->>FS: Write Cargo.toml
//!     Project->>FS: Write config/config.yaml
//!     Project->>FS: Copy openapi.yaml to doc/
//!     Project->>FS: Write static files
//!     Project-->>CLI: Success
//!     
//!     CLI-->>User: ✅ Generated project at output_dir
//! ```
//!
//! ### Request Handling Flow
//!
//! When a request arrives at a running BRRTRouter service, it flows through multiple layers:
//!
//! ```mermaid
//! sequenceDiagram
//!     participant Client
//!     participant Server as HttpServer<br/>(may_minihttp)
//!     participant Middleware as Middleware Chain
//!     participant Auth as AuthMiddleware
//!     participant ReqVal as Request Validator
//!     participant Router as Router
//!     participant Dispatcher as Dispatcher
//!     participant Handler as Handler<br/>(Coroutine)
//!     participant RespVal as Response Validator
//!
//!     Client->>Server: HTTP Request<br/>GET /pets/123
//!     Server->>Server: Parse HTTP<br/>(headers, body, query)
//!     
//!     Server->>Middleware: Pre-process request
//!     Middleware->>Middleware: CORS headers
//!     Middleware->>Middleware: Tracing span
//!     Middleware->>Auth: Validate credentials
//!     
//!     Auth->>Auth: Extract API key/JWT<br/>from headers/cookies
//!     Auth->>Auth: Lookup security provider
//!     
//!     alt Authentication Required
//!         Auth->>Auth: Validate token/signature
//!         alt Invalid Credentials
//!             Auth-->>Client: 401 Unauthorized
//!         end
//!         Auth->>Auth: Check scopes/permissions
//!         alt Insufficient Permissions
//!             Auth-->>Client: 403 Forbidden
//!         end
//!     end
//!     
//!     Auth-->>Middleware: ✓ Authenticated
//!     Middleware->>Middleware: Metrics (request count)
//!     Middleware-->>Server: Continue
//!     
//!     Server->>ReqVal: Validate request
//!     ReqVal->>ReqVal: Check required params
//!     ReqVal->>ReqVal: Validate param types
//!     ReqVal->>ReqVal: Check param constraints
//!     ReqVal->>ReqVal: Validate JSON body schema
//!     
//!     alt Validation Failed
//!         ReqVal-->>Client: 400 Bad Request<br/>(RFC 7807 Problem Details)
//!     end
//!     
//!     ReqVal-->>Server: ✓ Valid Request
//!     
//!     Server->>Router: match_route("GET", "/pets/123")
//!     Router->>Router: Test regex patterns
//!     Router->>Router: Extract path params<br/>{id: "123"}
//!     
//!     alt No Route Match
//!         Router-->>Client: 404 Not Found
//!     end
//!     
//!     Router-->>Server: RouteMatch<br/>(handler: "get_pet", params)
//!     
//!     Server->>Dispatcher: dispatch(handler_name, request)
//!     Dispatcher->>Dispatcher: Lookup handler coroutine
//!     
//!     alt Handler Not Registered
//!         Dispatcher-->>Client: 404 Handler Not Found
//!     end
//!     
//!     Dispatcher->>Handler: Send via channel<br/>(HandlerRequest)
//!     
//!     Note over Handler: Handler runs in<br/>may coroutine
//!     Handler->>Handler: Extract typed params
//!     Handler->>Handler: Business logic<br/>(DB query, etc.)
//!     
//!     alt Handler Panics
//!         Handler-->>Dispatcher: Panic caught
//!         Dispatcher-->>Client: 500 Internal Server Error
//!     end
//!     
//!     Handler-->>Dispatcher: HandlerResponse<br/>(status, headers, body)
//!     
//!     Dispatcher-->>Server: Response
//!     
//!     Server->>RespVal: Validate response
//!     RespVal->>RespVal: Check status code
//!     RespVal->>RespVal: Validate response schema
//!     RespVal->>RespVal: Check required fields
//!     
//!     alt Response Validation Failed
//!         RespVal->>RespVal: Log validation error
//!         Note over RespVal: Continue or fail<br/>based on config
//!     end
//!     
//!     RespVal-->>Server: ✓ Valid Response
//!     
//!     Server->>Middleware: Post-process response
//!     Middleware->>Middleware: Add security headers
//!     Middleware->>Middleware: Record metrics
//!     Middleware->>Middleware: End tracing span
//!     Middleware-->>Server: Final response
//!     
//!     Server-->>Client: HTTP Response<br/>200 OK + JSON body
//! ```
//!
//! ### Key Architectural Patterns
//!
//! 1. **OpenAPI-Driven**: All routing, validation, and types derived from the OpenAPI spec
//! 2. **Coroutine-Based Concurrency**: Each handler runs in a lightweight `may` coroutine
//! 3. **Channel Communication**: Requests routed to handlers via MPSC channels
//! 4. **Middleware Chain**: Request/response processing through composable middleware
//! 5. **Fail-Fast Validation**: Early validation of requests before reaching handlers
//! 6. **Template-Based Generation**: Code generation uses Askama templates for consistency
//!
//! ## Quick Start
//!
//! ```no_run
//! use brrtrouter::{load_spec, router::Router, server::AppService};
//!
//! // Load your OpenAPI specification
//! let spec = load_spec("openapi.yaml").expect("Failed to load spec");
//!
//! // Create a router from the spec
//! let router = Router::from_spec(&spec);
//!
//! // Create and start the service
//! let service = AppService::new(router, spec);
//! // service.start("0.0.0.0:8080");
//! ```
//!
//! ## Features
//!
//! - **OpenAPI-First**: Your API specification is the single source of truth
//! - **Coroutine-Powered**: Built on the `may` runtime for efficient concurrency
//! - **Type-Safe**: Optional typed handler support with automatic deserialization
//! - **Security**: Built-in support for API keys, JWT, OAuth2, and custom providers
//! - **Validation**: Automatic request/response validation against OpenAPI schemas
//! - **Code Generation**: Generate complete service scaffolding from your spec
//! - **Hot Reload**: Reload specifications without restarting your service
//! - **Middleware**: Extensible middleware system for cross-cutting concerns
//!
//! ## Runtime Considerations
//!
//! BRRTRouter uses the `may` coroutine runtime, not tokio or async-std. This means:
//!
//! - All handlers run in coroutines (lightweight threads)
//! - Stack size is configurable via `BRRTR_STACK_SIZE` environment variable
//! - The runtime is incompatible with tokio-based libraries without bridging
//! - Blocking operations should use `may`'s blocking facilities
//!
//! ## Code Generation
//!
//! BRRTRouter includes a code generator that creates complete service implementations:
//!
//! ```bash
//! cargo run --bin brrtrouter-gen -- generate --spec openapi.yaml --output my-service
//! ```
//!
//! This generates handlers, controllers, and a complete service structure ready to run.
//!
//! ## Example: Pet Store
//!
//! The repository includes a complete example application (`examples/pet_store`) that demonstrates
//! all features of BRRTRouter. This is a fully functional service generated from the Pet Store
//! OpenAPI specification.
//!
//! ### Generated Project Structure
//!
//! ```text
//! examples/pet_store/
//! ├── Cargo.toml              # Project dependencies
//! ├── config/
//! │   └── config.yaml         # Security and HTTP configuration
//! ├── doc/
//! │   ├── openapi.yaml        # OpenAPI 3.1 specification
//! │   └── openapi.html        # Rendered API documentation
//! ├── static_site/
//! │   └── index.html          # Landing page
//! └── src/
//!     ├── main.rs             # Service startup and configuration
//!     ├── registry.rs         # Handler registration with dispatcher
//!     ├── handlers/           # Business logic handlers
//!     │   ├── mod.rs
//!     │   ├── list_pets.rs    # GET /pets
//!     │   ├── add_pet.rs      # POST /pets
//!     │   ├── get_pet.rs      # GET /pets/{id}
//!     │   ├── list_users.rs   # GET /users
//!     │   ├── get_user.rs     # GET /users/{user_id}
//!     │   └── ...             # 22 handlers total
//!     └── controllers/        # Request/response controllers
//!         ├── mod.rs
//!         ├── list_pets.rs    # Calls handler, returns response
//!         ├── add_pet.rs
//!         └── ...             # One per handler
//! ```
//!
//! ### Running the Pet Store
//!
//! ```bash
//! # Start the service
//! cd examples/pet_store
//! cargo run -- \
//!   --spec doc/openapi.yaml \
//!   --doc-dir ../../examples/pet_store/doc \
//!   --config config/config.yaml \
//!   --test-api-key test123
//!
//! # Or use the justfile
//! just start-petstore
//! ```
//!
//! ### Example API Calls
//!
//! ```bash
//! # Health check (no auth required)
//! curl http://localhost:8080/health
//!
//! # List all pets (requires API key)
//! curl -H "X-API-Key: test123" http://localhost:8080/pets
//!
//! # Get a specific pet
//! curl -H "X-API-Key: test123" http://localhost:8080/pets/123
//!
//! # Add a new pet
//! curl -X POST \
//!   -H "X-API-Key: test123" \
//!   -H "Content-Type: application/json" \
//!   -d '{"name":"Fluffy","species":"Cat"}' \
//!   http://localhost:8080/pets
//!
//! # View metrics
//! curl http://localhost:8080/metrics
//!
//! # View OpenAPI documentation
//! open http://localhost:8080/doc/openapi.html
//! ```
//!
//! ### Features Demonstrated
//!
//! The Pet Store example showcases:
//!
//! - **22 API endpoints** - Full CRUD operations for pets, users, posts
//! - **Multiple HTTP methods** - GET, POST, PUT, DELETE, HEAD, OPTIONS
//! - **Path parameters** - `/pets/{id}`, `/users/{user_id}/posts/{post_id}`
//! - **Query parameters** - `/users?limit=10&offset=0`
//! - **Request body validation** - JSON schema validation for POST/PUT
//! - **Authentication** - API key in header, query, or cookie
//! - **Authorization** - Bearer JWT and OAuth2 examples
//! - **Parameter styles** - Simple, matrix, label styles
//! - **File uploads** - Multipart form data handling
//! - **Server-Sent Events** - `/events` endpoint for real-time updates
//! - **Static files** - Landing page and documentation
//! - **Metrics** - Prometheus-compatible metrics at `/metrics`
//! - **Health checks** - `/health` endpoint for monitoring
//! - **CORS support** - Cross-origin resource sharing
//! - **Hot reload** - Update OpenAPI spec without restart (development)
//!
//! ### Configuration
//!
//! The Pet Store is configured via `config/config.yaml`:
//!
//! ```yaml
//! security:
//!   api_keys:
//!     ApiKeyHeader:
//!       key: test123
//!       header_name: X-API-Key
//!   bearer:
//!     signature: mock_signature
//!   oauth2:
//!     signature: mock_signature
//!
//! http:
//!   keep_alive: true
//!   timeout_secs: 5
//!   max_requests: 1000
//! ```
//!
//! ### Handler Example
//!
//! Here's what a generated handler looks like (`src/handlers/get_pet.rs`):
//!
//! ```rust,ignore
//! use brrtrouter::dispatcher::HandlerResponse;
//! use crate::handlers::GetPetRequest;
//!
//! pub fn get_pet(req: GetPetRequest) -> HandlerResponse {
//!     // Extract path parameter
//!     let pet_id = req.id;
//!     
//!     // Business logic (database query, etc.)
//!     let pet = fetch_pet_from_db(&pet_id);
//!     
//!     // Return typed response
//!     HandlerResponse::ok_json(pet)
//! }
//! ```
//!
//! ### Regenerating the Example
//!
//! The Pet Store is auto-generated and can be regenerated at any time:
//!
//! ```bash
//! # Regenerate from OpenAPI spec
//! cargo run --bin brrtrouter-gen -- \
//!   generate \
//!   --spec examples/openapi.yaml \
//!   --force
//!
//! # Or use the justfile
//! just gen
//! ```
//!
//! **Important**: Do not edit files in `examples/pet_store` directly! They will be overwritten
//! on regeneration. Instead, modify the templates in `templates/` or the OpenAPI spec in
//! `examples/openapi.yaml`.

pub mod cli;

pub mod dispatcher;
mod dummy_value;
mod echo;
pub mod generator;
pub mod hot_reload;
pub mod middleware;
pub mod router;
pub mod runtime_config;
pub mod security;
pub mod server;
pub mod spec;
pub mod sse;
pub mod static_files;
pub mod typed;
pub mod validator;

pub use security::{BearerJwtProvider, OAuth2Provider, SecurityProvider, SecurityRequest};
pub use spec::{
    load_spec, load_spec_from_spec, load_spec_full, ParameterLocation, ParameterMeta,
    ParameterStyle, RouteMeta, SecurityRequirement, SecurityScheme,
};
