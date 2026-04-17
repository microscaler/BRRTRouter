# Codebase Entry Points

- Status: verified

## Runtime core
- Spec loading/building:
  - `src/spec/load.rs`
  - `src/spec/build.rs`
- Router matching:
  - `src/router/core.rs`
- Dispatch layer:
  - `src/dispatcher/core.rs`
- HTTP server wrapper:
  - `src/server/http_server.rs`
- Request service pipeline:
  - `src/server/service.rs`
- Middleware registry:
  - `src/middleware/mod.rs`
- Validation utilities:
  - `src/validator.rs`
- Hot reload watcher:
  - `src/hot_reload.rs`

## Generator core
- Generator API exports:
  - `src/generator/mod.rs`
- Template rendering + writer helpers:
  - `src/generator/templates.rs`
- Project generation orchestration:
  - `src/generator/project/generate.rs`
- CLI binary entrypoint:
  - `src/bin/brrtrouter_gen.rs`
