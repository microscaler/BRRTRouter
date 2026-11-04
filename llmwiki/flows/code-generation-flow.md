# Code Generation Flow

- Status: verified
- Source docs: `docs/ARCHITECTURE.md`, `docs/GENERATOR_IMPL_AND_DEPENDENCIES_ANALYSIS.md`, `docs/DEPENDENCIES_CONFIG_GUIDE.md`

## High-confidence flow (code-anchored)
1. CLI delegates to library CLI runner:
   - `src/bin/brrtrouter_gen.rs`
2. Generator orchestrator loads spec and computes slug:
   - `src/generator/project/generate.rs`
3. Component schemas + per-route request/response schema types are collected:
   - `src/generator/project/generate.rs`
   - `src/generator/schema.rs`
   - object schemas become Rust structs; top-level OpenAPI string enums become
     serde-renamed Rust enums with a deterministic first-variant default
4. Template writers generate handlers/controllers/registry/main/docs:
   - `src/generator/templates.rs`
5. Output is written into generated project directories (`src/`, `doc/`, `config/`, `static_site/`):
   - `src/generator/project/generate.rs`
6. Impl stub generation (`generate-stubs`) — separate from full `generate`:
   - `src/generator/project/generate.rs` — `generate_impl_stubs`, sentinels, `--sync`
   - **PRD:** [`docs/PRD_IMPL_CONTROLLER_LIFECYCLE.md`](../../docs/PRD_IMPL_CONTROLLER_LIFECYCLE.md) — Tier 1 registry + Tier 2 manifest merge
7. Impl registry regen (post-migration) — full disk discovery, never overwrites controller bodies:
   - `brrtrouter-gen regen-impl-registry --spec … --output …/impl [--apply]`
   - Wiki: [`topics/impl-controller-lifecycle-rollout.md`](../topics/impl-controller-lifecycle-rollout.md)

## Practical constraint
- `examples/pet_store/` is generated output; edit generator/templates then regenerate.

## Integer types: OpenAPI `format` → Rust (2026-11-04)

`schema::integer_type` picks the Rust type of every `type: integer` the generator maps: struct fields, nullable fields, array items, and through `schema_to_type` path/query/header/cookie parameters and `oneOf` variants.

| `format` | Rust |
|---|---|
| `int64` | `i64` |
| `uint32` | `u32` |
| `uint64` | `u64` |
| `int32`, narrower or unknown formats, none | `i32` |

- Before this, every integer was `i32`, so an `int64` field rejected values above `i32::MAX` (epoch ms, byte counts, volumes; epoch seconds from 2038).
- `minimum: 0` alone stays signed; only `uint32`/`uint64` select unsigned types. Give unsigned fields `minimum: 0` too, so body validation rejects negatives before the typed conversion does.
- Parameters: `request::decode_param_value` parses integers as `i64`, falling back to `u64` only for `uint64`. `param_validation` rejects a value outside its format's range (`request::integer_fits_format`) with `expected_integer`; a parameter without a format accepts any JSON integer.
- Specs should state a format on every integer: "no format" means `i32` in the generated code.
- Tests: `tests/integer_format_tests.rs`.
