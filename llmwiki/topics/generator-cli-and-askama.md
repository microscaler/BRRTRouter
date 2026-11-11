# `brrtrouter-gen` CLI and Askama templates

- **Status**: `verified`
- **Source docs**: [`flows/code-generation-flow.md`](../flows/code-generation-flow.md)
- **Code anchors**: `src/bin/brrtrouter_gen.rs`, `src/generator/`, `src/generator/templates.rs`, `templates/*.txt` (Askama templates for gen crates)
- **Last updated**: 2026-11-11

## Tier 1 impl commands (2026-07-08)

| Command | Purpose |
|---------|---------|
| `plan-impl` | Dry-run: list controllers that would register |
| `migrate-registration [--apply]` | First migration: `impl_registry.rs` + patch `main.rs` (scoped to old match arms) |
| `regen-impl-registry [--apply]` | Full disk discovery; **safe** for adding handlers like bidding `save_draft_quote` |
| `migrate-main [--apply]` | Collapse legacy `main.rs` to Fix B `RunAppBuilder` |

See [`topics/impl-controller-lifecycle-rollout.md`](../topics/impl-controller-lifecycle-rollout.md) — Fix B complete on all hauliage impls.

## Which generator runs, and formatting (2026-11-11)

`brrtrouter_tooling.gen.brrtrouter.generator_command` picks the `brrtrouter-gen` the tooling runs:

1. `BRRTROUTER_GEN=<binary>`: a deliberate local build.
2. `BRRTROUTER_GEN_LOCAL=1`: the sibling checkout (`cargo run`), whatever its rev.
3. The consumer pins `brrtrouter = { git = …, rev = … }` (its `Cargo.toml` or `microservices/Cargo.toml`): the binary
   at that rev under `BRRTROUTER_GEN_CACHE` (default `~/.cache/brrtrouter-gen/<rev>`); else the sibling checkout when
   its HEAD is that rev; else `cargo install --locked --git … --rev <rev>` into the cache, refused with an explanation
   when that fails.
4. No pin: the sibling checkout, as before.

Until 2026-11-11 the tooling always ran the sibling checkout, so generated code followed whatever that checkout held
rather than the rev the services compile against.

`regen-impl-registry --apply` formats the files it writes with `rustfmt --edition <crate edition>`
(`generator::format_file`, `generator::crate_edition`) instead of `cargo fmt`, which needs the crate inside its
workspace and otherwise left the registry unformatted.

## What it is

The **code generator** renders Rust crates (handlers, controllers, registry) from OpenAPI using **Askama** templates. Orchestration lives under `src/generator/`; the CLI entrypoint is **`src/bin/brrtrouter_gen.rs`**.

Consumer repos (e.g. Hauliage) run generation as part of their **service scaffold** workflow — see Hauliage [`PRD_BFF_SCAFFOLDING_REMEDIATION.md`](../../../hauliage/docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md) and [`hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md`](../../../hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md).

## Schema Type Coverage

`src/generator/schema.rs` collects both object schemas and top-level OpenAPI
string enums. `templates/handler_types.rs.txt` renders string enums with:

- sanitized, collision-safe Rust variant identifiers;
- `serde(rename = ...)` carrying the exact OpenAPI wire value;
- the first declared OpenAPI value as the Rust `Default` variant.

Regression coverage lives in `src/generator/tests.rs`, including wire-value
preservation, identifier sanitation, collisions, and rendered output.

Selective generation scopes are strict: `--only types` writes type definitions
without overwriting the embedded `doc/openapi.yaml`. The spec copy belongs to
the `docs` scope.

> **Open:** Numeric and mixed-value JSON Schema enums are not emitted as Rust
> enums. Add a representation policy and tests before a consumer relies on
> those forms.

## Cross-references

- [`reference/openapi-extensions.md`](../reference/openapi-extensions.md)
- [`flows/code-generation-flow.md`](../flows/code-generation-flow.md)
