# BRRTRouter — agent rules

> **Desktop dev environment** — before doing anything in this repo, read the
> Microscaler-wide topology brief. It explains that you are on a Mac but the
> code lives on `ms02` (NFS), where commands execute for this environment, how
> the Kind cluster and vLLM fit in, and the network constraints behind the SSH
> tunneling. Do not duplicate its contents here — link to it. If reality drifts,
> fix the canonical doc, not this copy.
>
> - GitHub: [`cylon-local-infra/docs/desktop-dev-environment.md`](https://github.com/microscaler/cylon-local-infra/blob/main/docs/desktop-dev-environment.md)
> - On ms02 NFS: `~/Workspace/microscaler/cylon-local-infra/docs/desktop-dev-environment.md`

---

Strict operational rules for AI assistants working in this repository. **Knowledge about how BRRTRouter works is in [`llmwiki/`](./llmwiki/), not here.** This file only holds rules the agent must obey.

---

## Before you do anything

1. Read [`llmwiki/index.md`](./llmwiki/index.md) — the content catalog.
2. Read [`llmwiki/SCHEMA.md`](./llmwiki/SCHEMA.md) — wiki conventions and workflows.
3. Tail [`llmwiki/log.md`](./llmwiki/log.md) for recent context from other sessions.
4. Read the specific topic / entity / reference pages relevant to your work. Drill into prose `docs/` or source only when the wiki flags drift or a gap.

Sessions that skip this waste work. The wiki is the compounding artifact that makes knowledge persist across sessions — see [Karpathy's llm-wiki gist](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f).

---

## Repository shape

- Primary language: Rust (workspace with `brrtrouter`, `brrtrouter_macros`, and `examples/pet_store`).
- UI demo: SolidJS + Vite in `sample-ui/`.
- Generated code: `examples/pet_store/` is auto-generated; do not edit directly.
- Sibling repos (typical `microscaler/` checkout): **`../hauliage/`** (primary HTTP consumer + BFF) — [`docs/llmwiki/`](../hauliage/docs/llmwiki/). **`../lifeguard/`** (ORM + migrations) — [`docs/llmwiki/`](../lifeguard/docs/llmwiki/). Use [`llmwiki/topics/sibling-repos-and-wikis.md`](./llmwiki/topics/sibling-repos-and-wikis.md) for a responsibility split.

---

## Build, lint, test commands

### Recommended entry points (justfile)

- `just dev-up` / `just dev-down` / `just dev-status` — local dev environment (kind + Tilt).
- `just gen` — Regenerate `examples/pet_store/` from `examples/openapi.yaml`.
- `just build-ui` — Build SolidJS dashboard into `examples/pet_store/static_site`.

### Build

- `cargo build` — Build default workspace.
- `cargo build -p pet_store` — Build the generated pet store example.
- `cargo build --release` — Release build (used by Tilt sync).

### Format / lint

- `cargo fmt` — Format Rust (always run before committing).
- `cargo clippy --workspace --all-targets --all-features` — Lint. Configured deny-lints in `Cargo.toml`.

### Test

- `just test` — `cargo test -- --nocapture`.
- `just nt` — Fast parallel tests with nextest (recommended).
- `cargo test --test server_tests` — Single integration test module.
- `cargo test router::tests::test_route_matching` — Single test by name.
- `cargo test -- --ignored` — Ignored tests.

### Coverage / bench / profiling

- `just coverage` — Coverage report (must stay ≥80%).
- `just bench` — Criterion benchmarks.
- `just flamegraph` — Generate flamegraphs.

### UI (sample dashboard)

- `yarn install` (in `sample-ui/`).
- `yarn dev` — Dev server.
- `yarn build:petstore` — Build into `examples/pet_store/static_site`.

---

## Core rules the agent must obey

### 1. Do not edit generated code

`examples/pet_store/` is regenerated from `examples/openapi.yaml`. Any edit will be clobbered. Fix the spec or the template under `templates/` instead, then regenerate with `just gen`.

### 2. Follow Rust conventions

- `snake_case` for fns / modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Group imports: std, external crates, local modules. Prefer explicit imports.
- `Result<T, E>` + `?` over `panic!` in library paths. No `unwrap()` / `expect()` in production (`clippy::unwrap_used` is deny).

### 3. Hot-path JSF-AV safety

- Avoid allocations on routing / dispatch hot paths (use `SmallVec`).
- Preallocate (`with_capacity`) when collections are needed.
- Keep dispatch paths deterministic; no panics.
- Unsafe is allowed but must be isolated and well-justified (document safety invariants).

### 4. Documentation expectations

- Public modules require `//!` module-level docs (overview, architecture, examples).
- Public fns / structs / enums / traits require `///` docs (purpose, args, returns, examples, panics / safety).
- Test modules have `//!` module docs explaining coverage and strategy.

### 5. Testing discipline

- Run `just nt` before submitting.
- Keep tests deterministic; avoid global state.
- Maintain ≥80% coverage; add tests for new behaviour.

---

## Commit discipline

- Commits follow Conventional Commits (`feat(scope):`, `fix(scope):`, `docs(scope):`, `chore(scope):`, `refactor(scope):`).
- **Never push** without explicit human authorization.
- **Never use `--no-verify`** or `--no-verify-commit`. Let pre-commit hooks run.
- **Never commit secrets** (`.env`, credentials, tokens).

---

## Useful files

- [`README.md`](./README.md) — project overview.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — contributor workflow.
- [`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md) — development workflow + `just` commands.
- [`docs/TEST_DOCUMENTATION.md`](./docs/TEST_DOCUMENTATION.md) — test suite breakdown.
- [`Cargo.toml`](./Cargo.toml) — workspace config + lint rules.
- [`docs/llmwiki/index.md`](./docs/llmwiki/index.md) — wiki catalog (performance, validation pipeline, bench harness).
- [`llmwiki/docs-catalog.md`](./llmwiki/docs-catalog.md) *(planned)* — inventory of `docs/` sources.

---

## Explicit instruction: read the wiki

**Every session starts with reading [`llmwiki/`](./llmwiki/).** This is not optional.

End-of-session: update the wiki pages your work touched, append a `log.md` entry, flag any `> **Open:**` questions. Leave the wiki one step more useful than you found it. The convention for BRRTRouter features is: every PRD / feature commit adds or extends 1–3 wiki pages tied to the work, cross-linked to hauliage ADRs where concepts span both repos.
