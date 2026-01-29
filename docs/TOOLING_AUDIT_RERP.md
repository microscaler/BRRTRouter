# RERP Tooling Audit: Reusable Functionality for BRRTRouter Consumers

**Objective:** Identify RERP tooling that is reusable by other consumers of BRRTRouter so they do not have to rebuild it. This document tabulates modules, tests, and recommendations (move to BRRTRouter vs stay in RERP).

**Goal:** Make it easier for others to consume BRRTRouter by moving generic tooling into `brrtrouter-tooling` and keeping only RERP-specific wiring in RERP.

**Migration status legend:** `Done` = in BRRTRouter; `Not started` = candidate, not yet migrated; `In progress` = migration underway; `N/A` = stay in RERP (no migration).

---

## 1. Executive Summary

| Category | Already in BRRTRouter | Candidate to move | Stay in RERP | Migration status |
|----------|------------------------|-------------------|--------------|-------------------|
| **Modules** | BFF, docker, release, ports, build (host_aware + microservices), openapi, ci, gen, discovery, bootstrap, tilt, pre_commit | — | CLI entry (`rerp`), RERP config, gen/regenerate | All Done; N/A |
| **Rationale** | Implemented and consumed by RERP via re-export/thin wrappers | Generic “BRRTRouter consumer” workflows (validate specs, fix paths, call brrtrouter-gen, bootstrap, Tilt/Kind) | Single `rerp` binary and RERP layout defaults | — |

**Already migrated to BRRTRouter (RERP consumes):**

- **bff** — generate-system (directory discovery, merge BFF OpenAPI)
- **docker** — generate-dockerfile, copy-binary, copy-artifacts, copy-multiarch, build-base, build-image-simple, build-multiarch, unpack-build-bins
- **release** — bump (Cargo version), notes (OpenAI/Anthropic)
- **ports** — layout, discovery, registry, validate, reconcile, fix-duplicates
- **build** — host_aware (ARCH_TARGETS, cargo/cross/zigbuild)
- **openapi** — validate, fix_operation_id, check_decimal_formats, fix_impl_controllers
- **ci** — patch_brrtrouter, fix_cargo_paths, fix_impl_dependencies, get_latest_tag, is_tag, validate_version
- **gen** — find_brrtrouter, call_brrtrouter_generate, call_brrtrouter_generate_stubs (RERP keeps regenerate locally)
- **discovery** — suites, sources (layout from ports.layout)
- **bootstrap** — microservice (configurable layout; RERP re-exports, uses default layout)
- **tilt** — setup_kind_registry, setup_persistent_volumes, setup, teardown, logs (RERP: thin wrappers)
- **pre_commit** — workspace_fmt (run fmt when workspace dir changed; configurable workspace_dir, fmt_argv, extra_check_dirs; RERP: just fmt-rust + entities)
- **build/microservices** — build_workspace_with_options, build_package_with_options (configurable workspace_dir, arch, release, gen_if_missing_callback; RERP: PACKAGE_NAMES + run_accounting_gen_if_missing)

---

## 2. Module-by-Module Audit

### 2.1 Already in BRRTRouter (RERP re-exports or thin wrappers)

| Module | Purpose | Tests (RERP) | Status | Migration status |
|--------|---------|--------------|--------|-------------------|
| **bff/** | BFF OpenAPI generation from directory discovery (`openapi/{system}/{svc}/openapi.yaml` → merged BFF) | test_cli (bff), test_bff (in BRRTRouter) | ✅ In BRRTRouter. RERP: re-export only. | Done |
| **docker/** | Generate Dockerfile, copy binaries, build base/image (simple + multiarch), unpack zip artifacts | test_docker_* (31 tests) | ✅ In BRRTRouter. RERP: thin wrappers + BINARY_NAMES. | Done |
| **release/** | Bump Cargo.toml version; generate release notes (OpenAI/Anthropic) | test_release_bump, test_release_notes, test_cli (release) | ✅ In BRRTRouter. RERP: re-export with workspace_cargo_toml. | Done |
| **ports** (ports.py + layout/discovery/registry/validate) | Port registry, validate/reconcile/fix-duplicates; discovery from helm, kind, Tiltfile, BFF config | test_ports (in BRRTRouter), test_cli (ports) | ✅ In BRRTRouter. RERP: re-export. | Done |
| **build/host_aware** | Host-aware Rust build (cargo/cross/zigbuild, multi-arch) | test_build_host_aware (ARCH_TARGETS, run, etc.) | ✅ In BRRTRouter. RERP: still has build/microservices locally. | Done |

---

### 2.2 Candidates to Move to BRRTRouter

| Module | Purpose | Why move | Config / notes | Migration status |
|--------|---------|----------|-----------------|-------------------|
| **openapi/validate** | Validate all `openapi.yaml` under a directory (YAML load, root is dict) | Any BRRTRouter consumer has OpenAPI specs to validate | `openapi_dir` param; no RERP coupling. | Done |
| **openapi/fix_operation_id** | Convert operationId camelCase → snake_case (matches BRRTRouter linter) | Aligns specs with BRRTRouter linter; useful for any consumer | Already references BRRTRouter linter; generic. | Done |
| **openapi/check_decimal_formats** | Check number fields have format decimal/money | Quality check for specs; reusable | Optional; generic. | Done |
| **openapi/fix_impl_controllers** | Fix impl controllers: f64 literals → Decimal | Matches BRRTRouter-generated impl pattern | Generic for gen/impl layout. | Done |
| **ci/patch_brrtrouter** | Replace path deps for BRRTRouter (and lifeguard) with git refs; run `cargo update` | Any consumer using BRRTRouter as path dep in CI | Configurable git refs / crate list. | Done |
| **ci/fix_cargo_paths** | Fix brrtrouter/brrtrouter_macros path in Cargo.toml to local BRRTRouter path | Needed after brrtrouter-gen for local dev | Configurable project_root and BRRTRouter path. | Done |
| **ci/fix_impl_dependencies** | Add to impl Cargo.toml deps used by gen crates | Standard gen/impl pattern for BRRTRouter | Generic. | Done |
| **ci/get_latest_tag** | Get latest release tag from GitHub API | Release/version workflows | Repo from env or arg. | Done |
| **ci/is_tag** | Check if GITHUB_REF is a tag (e.g. refs/tags/v*) | CI branching | Generic. | Done |
| **ci/validate_version** | Compare versions; prevent downgrade | Release workflows | Generic. | Done |
| **gen/brrtrouter** | find_brrtrouter, call_brrtrouter_generate, call_brrtrouter_generate_stubs | Any project that invokes brrtrouter-gen | Configurable brrtrouter path (default sibling BRRTRouter). | Done |
| **gen/regenerate** | regenerate_service, regenerate_suite_services (from OpenAPI) | “Regenerate all” from specs | RERP keeps; uses brrtrouter_tooling.gen. | Not started |
| **discovery/suites** | Suites with BFF, bff-suite-config paths, iter_bffs, suite_sub_service_names | Used by ports validate, gen, BFF | Layout from ports.layout (resolve_layout). | Done |
| **discovery/sources** | Discover port usages from helm, kind-config, Tiltfile, bff-suite-config, openapi | Feeds ports validate | Same layout; move with suites. | Done |
| **bootstrap/microservice** | Bootstrap crate from OpenAPI: BRRTRouter codegen, Dockerfile, Cargo, Tiltfile, port registry | High value for “new service from OpenAPI” | Make layout configurable (openapi_dir, workspace_dir, tiltfile, etc.); RERP keeps RERP layout. | Done |
| **tilt/setup_kind_registry** | Create/start local registry (localhost:5001), connect to kind network | Any Tilt + Kind user | Generic. | Done |
| **tilt/setup_persistent_volumes** | Apply k8s YAML for PVs | Any Tilt+K8s project | Configurable pv_paths. | Done |
| **tilt/setup** | Create dirs and Docker volumes; check docker/tilt | Generic Tilt preflight | Configurable dirs/volumes. | Done |
| **tilt/teardown** | Tilt down; optional remove images/volumes/system prune | Generic | Configurable service_names, container/image fns. | Done |
| **tilt/logs** | Tail Tilt logs for a component | Generic | Generic. | Done |
| **pre_commit/microservices_fmt** | Run `cargo fmt` in a workspace dir when it changed | Any Rust workspace | Configurable workspace_dir, fmt_argv, extra_check_dirs. | Done |
| **build/microservices** | build_microservices_workspace, build_microservice, PACKAGE_NAMES, run_accounting_gen_if_missing | Workspace build + optional “gen if missing” | Move “build workspace with package list” to BRRTRouter; BRRTRouter: build_workspace_with_options, build_package_with_options (gen_callback); RERP: PACKAGE_NAMES + run_accounting_gen_if_missing. | Done |

---

### 2.3 Stay in RERP

| Module | Purpose | Why stay | Migration status |
|--------|---------|----------|-------------------|
| **cli/main.py, cli/*.py** | `rerp` CLI entry and subcommand wiring | RERP-specific binary and UX; other consumers have their own CLI or use brrtrouter directly. | N/A |
| **ports.py** (current) | Re-export of brrtrouter_tooling.ports | Stays as re-export; no move. | N/A |
| **bff/generate_system.py** (current) | Re-export brrtrouter_tooling.bff | Stays as re-export. | N/A |
| **docker/* (current)** | Thin wrappers with base_image_name=rerp-base, BINARY_NAMES, etc. | Stays as thin wrappers + RERP-specific config. | N/A |
| **release/* (current)** | Re-export brrtrouter_tooling.release | Stays as re-export. | N/A |
| **build/microservices** (if not moved) | RERP accounting workspace + PACKAGE_NAMES + run_accounting_gen_if_missing | If we do not generalize “workspace build + package list” in BRRTRouter, this stays RERP-only. | N/A |

---

## 3. Test Audit

**Policy:** When a module is moved to BRRTRouter, its unit tests are moved to BRRTRouter tooling and the duplicate test files are removed from RERP. RERP keeps only tests that exercise the `rerp` CLI or RERP-specific behaviour (e.g. test_cli, test_docker_*, test_release_*, test_build_host_aware). **Current:** RERP tooling ~167 tests; BRRTRouter tooling ~200 tests (openapi, ci, gen, discovery, tilt, bootstrap, ports, bff, release, docker, build, etc.).

| Test file | What it tests | Recommendation | Migration status |
|-----------|----------------|-----------------|-------------------|
| **test_bootstrap_microservice** | Port registry, to_snake_case, derive_binary_name, load_openapi_spec, update_workspace_cargo_toml, update_tiltfile, run_bootstrap_microservice | **Move with bootstrap** to BRRTRouter; keep RERP-specific layout tests in RERP or parametrized. | Done (moved to BRRTRouter test_bootstrap.py; removed from RERP) |
| **test_build_host_aware** | ARCH_TARGETS, detect_host_architecture, should_use_zigbuild/cross, _determine_architectures, run; build_microservice, build_microservices_workspace | **Host_aware:** in BRRTRouter. **Microservices:** RERP tests call brrtrouter_tooling.build.workspace_build; BRRTRouter has test_build_workspace. | Done |
| **test_ci_fix_cargo_paths** | fix_cargo_toml, run | **Move with ci/fix_cargo_paths.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_ci_get_latest_tag** | get_latest_tag, run, retries, backoff | **Move with ci/get_latest_tag.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_ci_is_tag** | is_tag (GITHUB_REF) | **Move with ci/is_tag.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_ci_patch_brrtrouter** | find_cargo_tomls, find_matches, patch_file, run | **Move with ci/patch_brrtrouter.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_ci_validate_version** | compare_versions, validate_version, run, CLI | **Move with ci/validate_version.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_cli** | Main help, openapi/bff/docker/ports/release/ci/tilt/build CLI flows (many delegate to brrtrouter) | **Stay in RERP** (tests the `rerp` CLI). Unit tests for moved modules live in BRRTRouter. | N/A |
| **test_discovery**, **test_discovery_sources** | suites_with_bff, discover_helm, discover_tiltfile, etc. | **Move with discovery** to BRRTRouter. | Done (moved to BRRTRouter; removed from RERP) |
| **test_docker_*** | Docker run/validate/copy/build (via wrappers → brrtrouter) | **Stay in RERP** for CLI/integration; BRRTRouter already has unit tests for docker modules. | N/A |
| **test_openapi_validate** | validate_specs | **Move with openapi/validate.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_openapi_fix_operation_id** | is_snake_case, to_snake_case, find_openapi_files, process_file, run | **Move with openapi/fix_operation_id.** | Done (moved to BRRTRouter; removed from RERP) |
| **test_release_bump**, **test_release_notes** | Bump logic, notes (get_previous_tag, commits, OpenAI/Anthropic) | **Already in BRRTRouter** (release tests there); RERP tests can stay for CLI or be removed if redundant. | Done |
| **test_tilt** | setup_kind_registry, setup_persistent_volumes, setup, teardown, logs | **Move with tilt** to BRRTRouter. | Done (moved to BRRTRouter; removed from RERP) |

---

## 4. Tabular Summary: Move vs Stay

| RERP module | Move to BRRTRouter | Stay in RERP | Notes | Migration status |
|-------------|--------------------|--------------|--------|-------------------|
| bff | ✅ (done) | Re-export | — | Done |
| docker | ✅ (done) | Thin wrappers + BINARY_NAMES | — | Done |
| release | ✅ (done) | Re-export | — | Done |
| ports | ✅ (done) | Re-export | — | Done |
| build/host_aware | ✅ (done) | — | build/microservices still RERP | Done |
| openapi/validate | ✅ (done) | — | Generic spec validation | Done |
| openapi/fix_operation_id | ✅ (done) | — | Matches BRRTRouter linter | Done |
| openapi/check_decimal_formats | ✅ (done) | — | Optional quality check | Done |
| openapi/fix_impl_controllers | ✅ (done) | — | Gen/impl pattern | Done |
| ci/patch_brrtrouter | ✅ (done) | — | CI path→git for BRRTRouter | Done |
| ci/fix_cargo_paths | ✅ (done) | Thin wrapper (gen name/version) | Local path fix after gen | Done |
| ci/fix_impl_dependencies | ✅ (done) | — | Impl Cargo deps from gen | Done |
| ci/get_latest_tag | ✅ (done) | — | GitHub release tag | Done |
| ci/is_tag | ✅ (done) | — | GITHUB_REF is tag | Done |
| ci/validate_version | ✅ (done) | — | Version comparison | Done |
| gen/brrtrouter | ✅ (done) | — | Call brrtrouter-gen (find + generate + generate-stubs) | Done |
| gen/regenerate | ✅ Recommended | — | Regenerate from OpenAPI (RERP keeps; uses brrtrouter_tooling.gen) | Not started |
| discovery/suites, sources | ✅ (done) | — | Layout from ports.layout | Done |
| bootstrap/microservice | ✅ (done) | — | Configurable layout | Done |
| tilt/* | ✅ (done) | Thin wrappers (RERP dirs/volumes/naming) | Kind registry, PVs, setup, teardown, logs | Done |
| pre_commit (microservices_fmt) | ✅ (done) | Thin wrapper (just fmt-rust, entities) | cargo fmt in workspace | Done |
| build/microservices | ✅ (done) | Thin wrapper (PACKAGE_NAMES, run_accounting_gen_if_missing) | Workspace build + package list + optional gen | Done |
| cli/* | — | ✅ | RERP-specific `rerp` CLI | N/A |

---

## 5. Suggested Migration Order

1. ~~**OpenAPI**~~ — Done.
2. ~~**CI**~~ — Done.
3. ~~**gen**~~ — brrtrouter Done; regenerate stays in RERP (uses brrtrouter_tooling.gen).
4. ~~**Discovery**~~ — Done.
5. ~~**Tilt**~~ — Done.
6. ~~**Bootstrap**~~ — Done (microservice with configurable layout; RERP re-exports).
7. ~~**Pre-commit**~~ — Done (workspace_fmt; RERP uses just fmt-rust + entities).
8. **Build** — microservices: generic “build workspace with package list” + optional “gen if missing” — Done (workspace_build; RERP thin wrapper).

---

## 6. Layout and Configuration

- **BRRTRouter tooling** already uses a **layout** for ports (e.g. `openapi_dir`, `helm_values_dir`, `kind_config`, `tiltfile`, `port_registry`, `bff_suite_config_name`). New modules (discovery, bootstrap, openapi, tilt) should accept an optional **layout** or **project_root + paths** so that:
  - RERP keeps its layout (openapi/, microservices/, helm/rerp-microservice/values, etc.).
  - Other consumers can pass a different layout or paths.
- RERP-specific constants (e.g. BINARY_NAMES, PACKAGE_NAMES, base image name “rerp-base”) stay in RERP or in config passed into brrtrouter_tooling.

---

## 7. Document Metadata

- **Last updated:** 2026-01-28
- **RERP tooling:** `rerp/tooling/src/rerp_tooling/`
- **BRRTRouter tooling:** `BRRTRouter/tooling/src/brrtrouter_tooling/`
- **Living doc:** Update this file as modules are moved or decisions change.
- **Status:** All candidate modules migrated. gen/regenerate stays in RERP.

---

## 8. Next Steps (To-Do)

| # | Task | Scope | Status |
|---|------|--------|--------|
| 1 | **pre_commit/microservices_fmt** | In BRRTRouter: run_workspace_fmt. RERP: thin wrapper (just fmt-rust, entities). | Done |
| 2 | **build/microservices** | In BRRTRouter: build_workspace_with_options, build_package_with_options (gen_callback). RERP: thin wrapper (PACKAGE_NAMES + run_accounting_gen_if_missing). | Done |
| — | gen/regenerate | Stays in RERP; already uses brrtrouter_tooling.gen. | N/A |
