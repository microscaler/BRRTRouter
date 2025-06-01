# Contributing to BRRTRouter

Thank you for your interest in contributing! This project includes a code generator and example output. Please do **not** edit files under `examples/` manually; they are generated from templates.

## Development Workflow

1. Modify templates under `templates/` or the generator logic in `src/generator/`.
2. Regenerate the example project with:
   ```bash
   cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
   ```
   or simply run `just gen` if you have `just` installed.
3. Run `cargo fmt` and `cargo test` before submitting a pull request.

## Code Base Overview

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

For more details, consult the inline documentation in each module. Contributions that improve tests and documentation are highly appreciated!
