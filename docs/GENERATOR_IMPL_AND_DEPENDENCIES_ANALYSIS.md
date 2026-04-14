# Generator: Impl Directories and Cargo.toml Dependencies — Analysis

This document captures the implementation and behaviour of (1) **impl directory generation** and (2) **passing Cargo.toml dependencies into consumer projects** in BRRTRouter. The work was done via cross-repo coding; this analysis serves as the missing documentation.

---

## 1. Recent Commits Context (Jan 2026)

### 1.1 Generator and dependencies

| Commit    | Date   | Summary |
|----------|--------|---------|
| `58d3d8f` | Jan 28 | **feat(generator):** add `brrtrouter-dependencies.toml` and dependencies config support |

### 1.2 Money and decimal support (Jan 27)

- Format-based type detection for money/decimal types
- Money/currency tester in sample UI
- Rust money type implementation
- Money tester fixes (correct `/items/{id}` endpoint, format without API calls)
- Use `Decimal` for money format (Money lifetime incompatible with serde)
- Convert JSON numbers to `Decimal` in `rust_literal_for_example` for controller examples

### 1.3 npm → yarn (Jan 27)

- Migrate from npm to yarn; Justfile and Tiltfile use yarn; CI uses setup-node yarn support

### 1.4 Other

- TypeScript tests for sample-ui; docs workflow trigger; version-arg feature; Dependabot dependency bumps

---

## 2. Impl Directory Generation

### 2.1 Purpose

Generate an **impl crate** that contains stub controller implementations for each handler defined in the OpenAPI spec. The impl crate lives alongside the generated “consumer” crate (e.g. `pet_store` + `pet_store_impl`).

### 2.2 Entry point and CLI

- **Function:** `generate_impl_stubs()` in `src/generator/project/generate.rs`
- **CLI:** `brrtrouter gen stubs --spec <path> --output <impl_output_dir> [--component-name <name>] [--path <handler>] [--force]`

### 2.3 Directory layout

All paths are under `impl_output_dir` (e.g. `crates/bff_impl`):

```
impl_output_dir/
├── Cargo.toml          # Created only if missing
└── src/
    ├── main.rs         # Created only if missing
    └── controllers/
        ├── mod.rs      # Updated to declare each handler module
        ├── list_pets.rs
        ├── get_pet.rs
        └── ...
```

- **`impl_output_dir/src/`** — crate `src` directory
- **`impl_output_dir/src/controllers/`** — one `.rs` stub per handler; `mod.rs` lists all controller modules

### 2.4 Creation rules

| Item | When created/updated |
|------|------------------------|
| `src/`, `src/controllers/` | Created if they do not exist (`fs::create_dir_all` when `!impl_controllers_dir.exists()`) |
| `Cargo.toml` | Written only if it does not exist |
| `src/main.rs` | Written only if it does not exist |
| `src/controllers/<handler>.rs` | Created or overwritten per handler; `--force` controls overwrite |
| `src/controllers/mod.rs` | Updated on each stub generation via `update_impl_mod_rs()` |

Stubs are **user-owned** once created: without `--force`, existing stubs are skipped to avoid overwriting custom logic.

### 2.5 Component name

- **Explicit:** `--component-name` (e.g. `bff`) → impl crate name is `{component_name}_impl`
- **Derived:** from directory name by stripping trailing `_impl` (e.g. `bff_impl` → `bff`). Directory name must end with `_impl` if `--component-name` is not provided.

### 2.6 Code locations

| Concern | File | Approx. location |
|---------|------|------------------|
| Impl stub generation entry | `src/generator/project/generate.rs` | `generate_impl_stubs()` ~618–793 |
| Directory creation | `src/generator/project/generate.rs` | ~666–670 (`impl_src_dir`, `impl_controllers_dir`) |
| Stub path | `src/generator/project/generate.rs` | ~704 `impl_controllers_dir.join(format!("{handler}.rs"))` |
| Cargo.toml / main.rs creation | `src/generator/project/generate.rs` | ~674–685 |
| Impl Cargo.toml template | `src/generator/templates.rs` | `write_impl_cargo_toml()` ~1201–1283 |
| Impl Cargo.toml template file | `templates/impl_cargo.toml.txt` | — |
| mod.rs update | `src/generator/templates.rs` | `update_impl_mod_rs()` ~1319 |
| CLI dispatch | `src/cli/commands.rs` | `Commands::GenerateStubs` ~189–205 |

---

## 3. Cargo.toml Dependencies for Consumers

### 3.1 Purpose

Allow generated **consumer** projects (e.g. `examples/pet_store`) to declare extra Cargo dependencies via a config file (`brrtrouter-dependencies.toml`) that is read at generation time. Dependencies can be always-included or conditional on type detection (e.g. include `rust_decimal` when `rust_decimal::Decimal` appears in generated types).

### 3.2 Config file: `brrtrouter-dependencies.toml`

- **Location:** Typically next to the OpenAPI spec (e.g. same directory as `openapi.yaml`), or passed explicitly via `--dependencies-config`.
- **Example:** `templates/brrtrouter-dependencies.toml.example`

Structure:

- **`[dependencies]`** — always included in the generated consumer `Cargo.toml`
- **`[conditional]`** — each entry has a `detect` pattern; the dependency is included only if that pattern appears in generated types or route request/response fields

### 3.3 Loading and resolution

- **Module:** `src/generator/dependencies_config.rs`
- **Resolution order:**  
  1. Explicit path (e.g. `--dependencies-config <path>`) if the file exists  
  2. Auto-detect: `spec_dir/brrtrouter-dependencies.toml`  
  3. None (no config)

Functions:

- `resolve_config_path(explicit_path, spec_path)` — returns the path to use (or `None`)
- `auto_detect_config_path(spec_path)` — looks in the spec’s parent directory for `brrtrouter-dependencies.toml`
- `load_dependencies_config(config_path)` — reads and parses TOML into `DependenciesConfig`; returns `Ok(None)` if file does not exist

### 3.4 Where config is used

- **Only in project generation**, not in impl stub generation.
- In `generate_project_with_options()` in `src/generator/project/generate.rs`:
  1. **Load config** (~205–214): `resolve_config_path(dependencies_config_path, spec_path)` then `load_dependencies_config(path)`.
  2. **Resolve conditional deps** (~358–430): scan `schema_types` and route request/response fields for each conditional’s `detect` string; collect names of conditional deps to include.
  3. **Write consumer Cargo.toml** (~433–441): `write_cargo_toml_with_options(base_dir, slug, use_workspace_deps, None, version, deps_config.as_ref(), Some(&detected_conditional_deps))`.

### 3.5 Consumer Cargo.toml template

- **Template:** `templates/Cargo.toml.txt`
- **Template data:** `CargoTomlTemplateData` in `src/generator/templates.rs` (~76–92) includes:
  - `config_dependencies` — formatted `[dependencies]` from config
  - `config_conditional_dependencies` — formatted conditional deps that were detected

Rendering:

- Both workspace and non-workspace dependency branches iterate `config_dependencies` and `config_conditional_dependencies`, producing lines like `# From brrtrouter-dependencies.toml [dependencies] section` and the dependency entry.

### 3.6 Impl crate Cargo.toml and config

- **Impl crate Cargo.toml does not use the dependencies config.**
- `write_impl_cargo_toml(impl_output_dir, component_name)` only receives directory and component name.
- `templates/impl_cargo.toml.txt` has no `config_dependencies` or `config_conditional_dependencies`; it only declares the impl crate, the consumer crate path, brrtrouter/brrtrouter_macros, serde, serde_json, jemalloc.

So: extra deps from `brrtrouter-dependencies.toml` apply only to the **main generated consumer** project, not to the impl crate.

### 3.7 CLI

- **`brrtrouter gen`** accepts `--dependencies-config <path>`; this is passed as `dependencies_config_path` to `generate_project_with_options()`.
- **`brrtrouter gen stubs`** has no dependency-config option; impl Cargo.toml is generated without config.

### 3.8 Code locations (dependencies config)

| Concern | File | Notes |
|---------|------|--------|
| Config types and loading | `src/generator/dependencies_config.rs` | `DependenciesConfig`, `ConditionalDependency`, `load_dependencies_config`, `resolve_config_path`, `auto_detect_config_path` |
| Use in project generation | `src/generator/project/generate.rs` | ~205–214 load; ~358–430 detect conditionals; ~433–441 `write_cargo_toml_with_options` |
| Formatting and template data | `src/generator/templates.rs` | `format_dependency_spec`, `CargoTomlTemplateData`, ~903–941 build `config_dependencies` / `config_conditional_dependencies` |
| Consumer Cargo.toml template | `templates/Cargo.toml.txt` | Uses `config_dependencies` and `config_conditional_dependencies` |
| CLI | `src/cli/commands.rs` | `dependencies_config` on `Commands::Generate` ~61; passed to `generate_project_with_options` ~180 |

---

## 4. Summary Table

| Concern | Where it happens | Config / CLI |
|--------|------------------|---------------|
| Impl directory layout | `generate_impl_stubs()` | `gen stubs --spec --output [--component-name] [--path] [--force]` |
| Impl Cargo.toml | `write_impl_cargo_toml()` | No dependencies config; created only if missing |
| Consumer Cargo.toml | `write_cargo_toml_with_options()` during project generation | `brrtrouter-dependencies.toml` via `--dependencies-config` or auto-detect |
| Dependencies config | `dependencies_config::*` + project generation | Used only for main generated project, not impl crate |

---

## 5. Related docs

- `docs/DEPENDENCIES_CONFIG_GUIDE.md` — user-facing guide for the dependencies config file
- `docs/DEPENDENCY_CONFIG_OPTIONS.md` / `docs/DEPENDENCY_SYSTEM_SUMMARY.md` — dependency system overview
- `templates/brrtrouter-dependencies.toml.example` — example config

---

*This analysis was produced to capture behaviour implemented during cross-repo work where in-repo documentation was missing.*
