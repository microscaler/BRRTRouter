# brrtrouter_tooling helpers audit

Canonical utility helpers live in **`brrtrouter_tooling.helpers`**. Other modules re-export or call into this module. This document tabulates helper-style functions per top-level module and whether they are relocated or should stay local.

## Summary table

| Module     | File / area           | Function(s) / helpers                                      | Status / action                                                                 |
|-----------|------------------------|-------------------------------------------------------------|----------------------------------------------------------------------------------|
| **root**  | `helpers.py`           | `to_pascal_case`, `is_snake_case`, `to_snake_case`          | **Canonical** (text).                                                            |
| **root**  | `helpers.py`           | `load_yaml_spec`, `validate_openapi_spec`, `extract_readme_overview` | **Canonical** (spec/file).                                                        |
| **root**  | `helpers.py`           | `downstream_path`, `find_openapi_files`, `find_cargo_tomls` | **Canonical** (path). `find_cargo_tomls(root, exclude=...)` for release/bump.     |
| **root**  | `helpers.py`           | `read_file_or_default`                                      | **Canonical** (file). Used by release/notes.                                     |
| **root**  | `helpers.py`           | `compare_versions`, `fibonacci_backoff_sequence`             | **Canonical** (version / retry).                                                 |
| **root**  | `helpers.py`           | `default_binary_name`                                       | **Canonical** (naming).                                                          |
| **bff**   | `merge.py`, `generate*.py`, `generate.py` | Import from `brrtrouter_tooling.helpers` (downstream_path, load_yaml_spec, to_pascal_case, extract_readme_overview, validate_openapi_spec) | **Done** – canonical location is root helpers only; no bff.helpers re-export.   |
| **bootstrap** | `helpers.py`        | `derive_binary_name`, `_get_registry_path`, `_get_port_from_registry` | **Done** – bootstrap uses root `load_yaml_spec` directly; no alias. |
| **bootstrap** | `microservice.py`   | Uses bootstrap.helpers                                      | **Done**.                                                                        |
| **release** | `bump.py`            | `_cargo_toml_paths`, `_read_current`, `_next_version`, `_replace_in_file`, `_set_workspace_package_version` | **Relocate path only**: use `find_cargo_tomls(root, exclude=SKIP_PARTS)`. Rest stay (version-bump domain). |
| **release** | `notes.py`           | `_load_template(path \| None) -> str`                       | **Done** – uses root `read_file_or_default`.                                     |
| **ci**    | `__init__.py`          | Re-exports `find_cargo_tomls`, `compare_versions` from root  | **Done** – ci package sources from brrtrouter_tooling.helpers.                    |
| **ci**    | `patch_brrtrouter.py`, `validate_version.py`, `get_latest_tag.py` | Use root helpers; __init__ re-exports from root             | **Done**.                                                                        |
| **openapi** | `fix_operation_id.py` | Already uses root `find_openapi_files`, `is_snake_case`, `to_snake_case` | **Done**.                                                                        |
| **docker** | `copy_multiarch.py`   | Already uses root `default_binary_name`                    | **Done**.                                                                        |
| **ports** | `layout.py`            | `resolve_layout`, `DEFAULT_LAYOUT`                          | **Keep** – ports-domain layout.                                                  |
| **discovery** | `suites.py`, `sources.py` | Use `ports.layout.resolve_layout`; suite/BFF discovery     | **Keep** – discovery domain.                                                     |
| **build** | `host_aware.py`        | `detect_host_architecture`, `should_use_zigbuild`, etc.    | **Keep** – build domain.                                                         |
| **gen**   | `brrtrouter.py`       | `find_brrtrouter`, `call_brrtrouter_generate*`              | **Keep** – gen domain.                                                           |
| **pre_commit** | `workspace_fmt.py`  | `_run`, `run_workspace_fmt`                                 | **Keep** – pre_commit domain.                                                    |
| **dependabot** | `automerge.py`      | GitHub/PR helpers                                           | **Keep** – dependabot domain.                                                    |
| **tilt**  | `setup*.py`, `teardown.py`, `logs.py` | `run` entrypoints                            | **Keep** – tilt/K8s domain.                                                      |
| **cli**   | Various                | Arg parsing and `run_*_argv`                                | **Keep** – CLI layer.                                                            |

## Relocations to perform

1. **`find_cargo_tomls`**  
   - Extend in `brrtrouter_tooling.helpers` to accept optional `exclude: set[str] | None = None` (default current: `target`, `node_modules`, `.git`).  
   - In `release/bump.py`: replace `_cargo_toml_paths` with `find_cargo_tomls(project_root, exclude=SKIP_PARTS)` and remove `_cargo_toml_paths`.

2. **`read_file_or_default`**  
   - Added to root helpers; `release/notes.py` uses it for `_load_template`.

## Usage convention

- **New generic helpers** (text, path, spec load, version, retry, naming): add to `brrtrouter_tooling.helpers`.
- **Module-specific helpers**: keep in the module; import root helpers where needed.
- **Re-exports**: `bootstrap.helpers` re-exports root helpers where needed; BFF code imports from `brrtrouter_tooling.helpers` only.
