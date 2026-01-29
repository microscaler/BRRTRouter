# BRRTRouter Tooling

Development tooling for BRRTRouter project automation.

## Installation

```bash
cd tooling
pip install -e ".[dev]"
```

## Usage

```bash
brrtrouter dependabot <command>
brrtrouter bff generate --suite-config <path> [--output <path>] [--base-dir <path>] [--validate]
brrtrouter bff generate-system [--openapi-dir <path>] [--system <name>] [--output <path>]
brrtrouter ports validate [--project-root <path>] [--registry <path>] [--json]
brrtrouter build <target> [arch] [--release] [--workspace-dir microservices]
brrtrouter docker <cmd> ...   # generate-dockerfile, copy-binary, build-base, build-image-simple, copy-multiarch, build-multiarch, unpack-build-bins
brrtrouter release bump [patch|minor|major|rc|release]
brrtrouter release generate-notes --version X.Y.Z [--output PATH] [--template PATH] [--since-tag TAG]
```

### BFF generator (Story 1.4)

Generates a BFF OpenAPI spec from a suite config YAML that lists sub-services. Implements Stories 1.2 (proxy extensions) and 1.3 (components/security merge).

- **Suite config:** `openapi_base_dir`, `output_path`, `services` (name → `base_path`, `spec_path`). Optional `metadata` (title, version, `security_schemes`, `security`).
- **Paths** in config are relative to `--base-dir` (default: current working directory).
- **Output:** Merged spec with `x-brrtrouter-downstream-path`, `x-service`, `x-service-base-path` on each operation; merged `components.parameters`, `components.securitySchemes`, root `security` when provided in config.
- **generate-system:** Directory-based discovery: `openapi_dir/{system}/{service}/openapi.yaml` → merged BFF at `openapi_dir/{system}/openapi.yaml`. Same merge logic as suite-config; RERP uses this via re-export from `brrtrouter_tooling.bff`.
- **ports validate:** Scan port-registry, helm values, kind-config, Tiltfile, bff-suite-config; report conflicts. RERP-style default layout (configurable). RERP re-exports `PortRegistry`, `validate`, `reconcile`, `fix_duplicates` from `brrtrouter_tooling.ports`.
- **build:** Host-aware Rust build (cargo/cross/zigbuild) for workspace or `<system>_<module>`. Configurable `--workspace-dir` (default: microservices).
- **docker:** Generate Dockerfile from template, copy binaries, build base/image (simple or multiarch), unpack build bins. RERP consumes with `base_image_name="rerp-base"`, `build_cmd`, etc.
- **release:** Bump Cargo.toml versions (configurable workspace path); generate release notes via OpenAI/Anthropic.
- **Consuming from GitHub:** `pip install "brrtrouter-tooling @ git+https://github.com/microscaler/BRRTRouter.git#subdirectory=tooling"`

## Development

### Linting

```bash
ruff check src/ tests/
ruff format src/ tests/
```

### Testing

```bash
pytest
```

### Building

```bash
pip install -e .
```
