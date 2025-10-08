# Contributing

Thank you for your interest in contributing! This project includes a code generator and example output. Please do **not** edit files under `examples/` manually; they are generated from templates.

Thank you for helping improve **BRRTRouter**! The example application in
`examples/pet_store` is automatically generated from `examples/openapi.yaml`.

The generator logic lives in `src/generator` and uses templates from
`templates/`.

## Development Workflow

1. Modify templates under `templates/` or the generator logic in `src/generator/`.
2. Regenerate the example project with:
   ```bash
   cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
   ```
   or simply run `just gen` if you have `just` installed.

`examples/pet_store` is automatically generated from
`examples/openapi.yaml`.

Direct edits to files inside `examples/pet_store` will be overwritten the
next time the generator runs.

## Updating the generated examples

1. Update the templates or generator code.
2. Run the generator:

   ```bash
   cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
   ```
   *(or run `just gen`)*
3. Commit any regenerated files as part of your change.

4. Run `cargo fmt` and `cargo test` before submitting a pull request.

## Documentation Standards

BRRTRouter follows strict documentation standards to help contributors understand the codebase.

### Module Documentation

Every public module must have module-level documentation (`//!`) that includes:

1. **Title** - A clear, concise module name
2. **Overview** - What the module does and why it exists
3. **Architecture** - How the module works (diagrams welcome)
4. **Usage Examples** - At least one practical example
5. **Key Types** - Links to important types exported by the module

Example:

```rust
//! # Router Module
//!
//! Provides path matching and route resolution for BRRTRouter.
//!
//! ## Overview
//!
//! The router matches incoming requests to handlers defined in OpenAPI specs...
//!
//! ## Example
//!
//! ```rust
//! use brrtrouter::router::Router;
//! let router = Router::from_spec(&spec);
//! ```
```

### Function and Type Documentation

All public functions, structs, enums, and traits must have doc comments (`///`) that include:

1. **Purpose** - What the item does
2. **Arguments** - Description of each parameter (for functions)
3. **Returns** - What the function returns
4. **Examples** - Code examples for non-trivial items
5. **Panics** - Document panic conditions
6. **Safety** - Document unsafe requirements (if applicable)

Example:

```rust
/// Loads an OpenAPI specification from a file.
///
/// # Arguments
///
/// * `path` - Path to the OpenAPI YAML or JSON file
///
/// # Returns
///
/// Returns the parsed `Spec` object or an error if parsing fails.
///
/// # Example
///
/// ```rust
/// let spec = load_spec("openapi.yaml")?;
/// ```
pub fn load_spec(path: &str) -> Result<Spec, Error> {
    // ...
}
```

### Generating Documentation

Generate and view the documentation locally:

```bash
# Using just (recommended)
just docs              # Generate and open docs with Mermaid diagrams
just docs-build        # Generate without opening
just docs-check        # Check for warnings and broken links

# Or using cargo directly
cargo doc --no-deps --lib --open

# The project is configured to automatically include Mermaid.js for diagram rendering
# via .cargo/config.toml and doc/head.html
```

**Note**: The documentation includes interactive Mermaid sequence diagrams. These are automatically rendered when you view the docs in a browser thanks to the Mermaid.js library loaded via `doc/head.html`.

**Quick Reference:**
- `just docs` - Most common: build and view docs
- `just docs-check` - Verify docs before committing
- Docs auto-include Mermaid.js for diagram rendering

### CI Checks

The CI pipeline automatically checks:
- Missing documentation warnings
- Broken intra-doc links
- Example code in docs compiles (when not marked as `ignore`)

### Test Documentation

Test modules should have a module-level comment explaining:
- What is being tested
- Test coverage scope
- Any special setup or teardown

Example:

```rust
//! # Router Tests
//!
//! Tests for path matching, parameter extraction, and route resolution.
//!
//! Coverage:
//! - Path parameter extraction
//! - Query parameter parsing
//! - HTTP method matching
//! - 404 handling
```

## Code Base Overview

### General Structure

**README overview** – BRRTRouter aims to be a “high‑performance, coroutine‑powered request router for Rust” driven entirely by an OpenAPI 3.1 specification. The vision calls for “millions of requests per second” with a goal of one million route matches per second on a Raspberry Pi 5.

**Crate layout** – The library is defined in `src/` and exposes modules such as `router`, `dispatcher`, `server`, `spec`, `typed`, etc. (`lib.rs` re‑exports several of them). There is a small CLI binary `src/bin/brrrouter_gen.rs` which just invokes `brrrouter::cli::run_cli()`.

**OpenAPI specification parsing** – `spec.rs` reads a spec file (JSON or YAML) and produces `RouteMeta` values describing HTTP method, path, handler name, request/response schemas, and examples. The `build_routes` function walks the spec’s paths and operations to create `RouteMeta` entries and captures JSON schema info where available.

**Router** – `router.rs` converts each OpenAPI path into a regular expression and stores the method + regex. At runtime `Router::route` matches a (method, path) pair to a `RouteMatch` containing path parameters and handler metadata.

**Dispatcher** – `dispatcher.rs` registers handler functions under string names and spawns a coroutine (using the may runtime) for each. Incoming requests are dispatched through channels to the registered handler. If a handler panics, the dispatcher returns a 500 error response. The `dispatch` method forwards a request and waits for the handler’s reply.

**HTTP server** – `server.rs` implements the `may_minihttp::HttpService` trait. It parses query parameters and JSON bodies, then uses the router and dispatcher to produce a JSON response or a 404/500 fallback.

**Typed handlers** – `typed.rs` offers a higher‑level interface: `Dispatcher::register_typed` automatically deserializes request bodies into strongly typed structures and serializes responses. It defines `TypedHandlerRequest<T>` and `TypedHandlerResponse<T>` as generics for typed data.

**Code generator** – `generator.rs` reads an OpenAPI spec and writes an example project under `examples/`. Templates in `templates/` define the generated `Cargo.toml`, handler stubs, controller stubs, and registry. See `generate_project_from_spec` which writes these files and creates typed structs from schema definitions.

**Examples and tests** – `examples/` contains an OpenAPI spec (`openapi.yaml`) and a generated “pet_store” example. Unit tests in `tests/router_tests.rs` focus on route matching for various HTTP verbs and paths. The README demonstrates running the server with `cargo run` and hitting it via `curl`, as well as running the tests with `cargo test -- --nocapture`.

### Module Overview

The `src/` directory is organized into several modules:

- **`cli`** – command-line interface and entry points for the generator.
- **`dispatcher`** – coroutine-based dispatcher that invokes handlers for matched routes.
- **`router`** – path matcher that builds a routing table from the OpenAPI spec.
- **`server`** – lightweight HTTP server built on `may_minihttp` plus request/response types.
- **`middleware`** – pluggable middleware such as metrics, CORS, and security hooks.
- **`security`** – implementations of bearer and OAuth2 security providers.
- **`generator`** – reads templates and the OpenAPI spec to produce example projects.
- **`spec`** – OpenAPI 3.1 parser used by the router and generator.
- **`typed`** – traits for typed request/response handlers.
- **`runtime_config`** – loads runtime options from environment variables.
- **`sse`** and **`static_files`** – helpers for server-sent events and serving static assets.

Key components include:

- `Router` – performs regex-based path matching and extracts path parameters.
- `Dispatcher` – coordinates coroutine execution of request handlers.
- `HttpServer` – drives the request/response loop and integrates middleware.

### Important Things to Know

- **OpenAPI‑driven** – The router relies entirely on the OpenAPI spec to define routes and handler names. Handler functions must be registered with exactly those names.
- **Coroutine runtime** – The project uses the may crate for lightweight coroutines and may_minihttp for serving HTTP. The project is generally incompatible with Tokio and an async bridge will need to be implemented specifically for Otel tracing.
- **Safety** – Handler registration uses unsafe (`register_handler` and `register_typed`) because the caller must guarantee the handler is safe in a concurrent environment.
- **Code generation** – The CLI (`brrrouter-gen`) can generate starter projects from a spec using Askama templates. This includes request/response structs, handler stubs, and controllers.
- **Testing focus** – Current unit tests verify the router’s matching logic for all HTTP verbs and for unknown paths.

### Pointers for Next Steps

- **Understand the spec module** – Learning how `spec.rs` parses the OpenAPI file and how `RouteMeta` is constructed is key to extending or validating new spec features.
- **Explore coroutine runtimes** – Look into the may crate to understand how coroutines and channels work, since handlers are expected to run in those coroutines.
- **Study the generator templates** – To customize generated code, review the templates under `templates/` (for example `handler.rs.txt` and `controller.rs.txt`) which show how typed handler modules are produced.
- **Run the example** – Try `cargo run` with the provided `examples/openapi.yaml` to see the router and echo handler in action.
- **Examine the unit tests** – `tests/router_tests.rs` illustrates how to parse a spec and check route matching; it’s a good starting point for adding more tests.

This repository demonstrates a minimal but modular OpenAPI-driven router. Once comfortable with the basics, explore improving typed request/response deserialization, dynamic handler registration, and the advanced features listed in the README’s “Contributing & Benchmarks” section.

For more details, consult the inline documentation in each module. Contributions that improve tests and documentation are highly appreciated!