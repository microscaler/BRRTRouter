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
```

### BFF generator (Story 1.4)

Generates a BFF OpenAPI spec from a suite config YAML that lists sub-services. Implements Stories 1.2 (proxy extensions) and 1.3 (components/security merge).

- **Suite config:** `openapi_base_dir`, `output_path`, `services` (name → `base_path`, `spec_path`). Optional `metadata` (title, version, `security_schemes`, `security`).
- **Paths** in config are relative to `--base-dir` (default: current working directory).
- **Output:** Merged spec with `x-brrtrouter-downstream-path`, `x-service`, `x-service-base-path` on each operation; merged `components.parameters`, `components.securitySchemes`, root `security` when provided in config.
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
