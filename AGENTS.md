# BRRTRouter Agent Notes
# Purpose: quick-start reference for agentic coding assistants

## Repository Overview
- Primary language: Rust (workspace with `brrtrouter`, `brrtrouter_macros`, and `examples/pet_store`).
- UI demo: SolidJS + Vite in `sample-ui/`.
- Generated code: `examples/pet_store/` is auto-generated; do not edit directly.

## Build, Lint, Test Commands

### Recommended entry points (justfile)
- `just dev-up` - Start local dev environment (kind + Tilt).
- `just dev-down` - Tear down local dev environment.
- `just dev-status` - Check cluster/pods/services status.
- `just gen` - Regenerate `examples/pet_store/` from `examples/openapi.yaml`.
- `just build-ui` - Build SolidJS dashboard to `examples/pet_store/static_site`.

### Build
- `cargo build` - Build default workspace.
- `cargo build -p pet_store` - Build the generated pet store example.
- `cargo build --release` - Release build (used by Tilt sync).

### Format / Lint
- `cargo fmt` - Format Rust code (always run before committing).
- `cargo clippy --workspace --all-targets --all-features` - Lint (lints are configured in `Cargo.toml`).

### Test (full suite)
- `just test` - Run `cargo test -- --nocapture`.
- `just nt` - Fast parallel tests with nextest (recommended).
- `cargo test -- --nocapture` - Standard tests with output.

### Run a single test or module
- `cargo test --test server_tests` - Run a specific integration test module.
- `cargo test router::tests::test_route_matching` - Run a specific test by name.
- `cargo test -- --ignored` - Run ignored tests.

### Coverage / Bench / Profiling
- `just coverage` - Coverage report (must stay ≥80%).
- `just bench` - Criterion benchmarks.
- `just flamegraph` - Generate flamegraphs.

### UI (sample dashboard)
- `npm install` (in `sample-ui/`)
- `npm run dev` (in `sample-ui/`) - Dev server.
- `npm run build:petstore` (in `sample-ui/`) - Build into `examples/pet_store/static_site`.

## Code Style and Conventions

### Rust formatting
- Use `cargo fmt` and keep rustfmt defaults.
- Keep line lengths reasonable; let rustfmt wrap.

### Imports
- Group imports by standard library, external crates, then local crate modules.
- Prefer explicit imports over glob unless re-exporting.
- Keep unused imports out; clippy warnings matter.

### Types and naming
- Follow Rust conventions: `snake_case` for functions/modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Prefer domain-specific names over generic ones (especially in generated handler code).
- Keep public APIs consistent with existing module naming patterns.

### Error handling
- Prefer `Result<T, E>` and error propagation (`?`) over `panic!`.
- Avoid `unwrap()`/`expect()` in production paths (clippy warns on these).
- Use structured errors via `anyhow` for CLI and top-level flows.

### JSF AV safety guidance (hot path)
- Avoid allocations in hot-path routing/dispatch code (use stack allocations like `SmallVec`).
- Keep dispatch paths deterministic and avoid panics.
- Prefer preallocation (`with_capacity`) when collections are necessary.

### Unsafe code
- Unsafe is allowed but should be isolated and well-justified.
- Prefer safe wrappers and document safety invariants.

### Documentation expectations
- Public modules require module-level docs (`//!`) with overview, architecture, and examples.
- Public functions/structs/enums/traits require `///` docs (purpose, args, returns, examples, panics/safety).
- Test modules should have `//!` module docs explaining coverage and strategy.

### Generated code workflow
- Do not edit `examples/pet_store/` directly.
- Update templates in `templates/` or generator logic in `src/generator/`.
- Regenerate with `just gen` and commit template + generated changes together.

### Testing discipline
- Run `just nt` before submitting changes.
- Keep tests deterministic; avoid global state where possible.
- Maintain ≥80% coverage; add tests for new behavior.

## Useful Files
- `README.md` - Project overview and links to docs.
- `CONTRIBUTING.md` - Contributor workflow and documentation standards.
- `docs/DEVELOPMENT.md` - Development workflow and just commands.
- `docs/TEST_DOCUMENTATION.md` - Test suite breakdown and execution.
- `Cargo.toml` - Workspace configuration and lint rules.

## Cursor/Copilot Rules
- No Cursor or Copilot rules were found in `.cursor/rules/`, `.cursorrules`, or `.github/copilot-instructions.md`.
