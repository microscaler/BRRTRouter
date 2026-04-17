# Codebase Entry Points

- Status: verified

## Runtime core
- Spec loading/building:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/spec/load.rs`
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/spec/build.rs`
- Router matching:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/router/core.rs`
- Dispatch layer:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/dispatcher/core.rs`
- HTTP server wrapper:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/server/http_server.rs`
- Request service pipeline:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/server/service.rs`
- Middleware registry:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/middleware/mod.rs`
- Validation utilities:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/validator.rs`
- Hot reload watcher:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/hot_reload.rs`

## Generator core
- Generator API exports:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/generator/mod.rs`
- Template rendering + writer helpers:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/generator/templates.rs`
- Project generation orchestration:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/generator/project/generate.rs`
- CLI binary entrypoint:
  - `/home/runner/work/BRRTRouter/BRRTRouter/src/bin/brrtrouter_gen.rs`
