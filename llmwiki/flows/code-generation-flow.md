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
4. Template writers generate handlers/controllers/registry/main/docs:
   - `src/generator/templates.rs`
5. Output is written into generated project directories (`src/`, `doc/`, `config/`, `static_site/`):
   - `src/generator/project/generate.rs`

## Practical constraint
- `examples/pet_store/` is generated output; edit generator/templates then regenerate.
