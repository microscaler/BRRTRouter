# BRRTRouter Tooling

Development tooling for BRRTRouter project automation.

## Installation

**Recommended (shared venv, same path used by PriceWhisperer / Hauliage Tiltfiles):**

```bash
python3 -m venv ~/.local/share/brrtrouter/venv
~/.local/share/brrtrouter/venv/bin/pip install -U pip
~/.local/share/brrtrouter/venv/bin/pip install -e ./tooling[dev]
```

Override the directory with **`BRRTROUTER_VENV`** (absolute path to the venv root, the directory that contains `bin/brrtrouter`).

**In-repo venv** (e.g. CI or quick try): `cd tooling && python3 -m venv .venv && .venv/bin/pip install -e ".[dev]"` — still supported; GitHub Actions uses a workspace-local venv.

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
brrtrouter mcp serve [--transport stdio|sse] [--host HOST] [--port PORT]
```

### Consumer workspace tooling (`brrtrouter_tooling.workspace`)

Optional helpers for **any** downstream repo that uses BRRTRouter (not tied to a single product name). This includes port registry, Tilt lifecycle scripts, Docker helpers, and a second-layer CLI (e.g. the `hauliage` command in Microscaler) that wires argparse and **optional** OpenAPI layout overrides (e.g. flattened `openapi/<service>/` trees). Prefer **`brrtrouter client …`** for portable scripts; use **`workspace`** when you need the full project-oriented command surface. Same package installs from PyPI/Git as `brrtrouter-tooling`; configure with `BRRTROUTER_ROOT`, `BRRTROUTER_VENV`, and project root env vars documented in `workspace/cli/main.py`.

### BFF generator (Story 1.4)

Generates a BFF OpenAPI spec from a suite config YAML that lists sub-services. Implements Stories 1.2 (proxy extensions) and 1.3 (components/security merge).

- **Suite config:** `openapi_base_dir`, `output_path`, `services` (name → `base_path`, `spec_path`). Optional `metadata` (title, version, `security_schemes`, `security`).
- **Paths** in config are relative to `--base-dir` (default: current working directory).
- **Output:** Merged spec with `x-brrtrouter-downstream-path`, `x-service`, `x-service-base-path` on each operation; merged `components.parameters`, `components.securitySchemes`, root `security` when provided in config.
- **generate-system:** Directory-based discovery: `openapi_dir/{system}/{service}/openapi.yaml` → merged BFF at `openapi_dir/{system}/openapi.yaml`. Same merge logic as suite-config; RERP uses this via re-export from `brrtrouter_tooling.bff`.
- **ports validate:** Scan port-registry, helm values, kind-config, Tiltfile, bff-suite-config; report conflicts. RERP-style default layout (configurable). RERP re-exports `PortRegistry`, `validate`, `reconcile`, `fix_duplicates` from `brrtrouter_tooling.ports`.
- **build:** Host-aware Rust build (cargo/cross/zigbuild) for workspace or `client build <system>_<module>`. Default `-p` is `{snake}_service_api_impl`, or `{Module}_impl` for camelCase BFF modules. Optional `--package` expands `foo_impl` or passes `rerp_*` through. Configurable `--workspace-dir` (default: microservices).
- **docker:** Generate Dockerfile from template, copy binaries, build base/image (simple or multiarch), unpack build bins. RERP consumes with `base_image_name="rerp-base"`, `build_cmd`, etc.
- **release:** Bump Cargo.toml versions (configurable workspace path); generate release notes via OpenAI/Anthropic.
- **mcp serve:** Start the BRRTRouter MCP server (see below).
- **Consuming from GitHub:** `pip install "brrtrouter-tooling @ git+https://github.com/microscaler/BRRTRouter.git#subdirectory=tooling"`

### MCP server

The `brrtrouter mcp serve` command starts a [Model Context Protocol](https://modelcontextprotocol.io/) server that helps AI assistants (Claude Desktop, Cursor, VS Code Copilot, etc.) build OpenAPI specs conformant to BRRTRouter, use the code generator, and set up BFF services.

**Installation** (requires the `mcp` extra). Prefer the shared venv above, then from a BRRTRouter clone:

```bash
cd /path/to/BRRTRouter
~/.local/share/brrtrouter/venv/bin/pip install -e "./tooling[mcp]"
# or from GitHub (any cwd):
~/.local/share/brrtrouter/venv/bin/pip install "brrtrouter-tooling[mcp] @ git+https://github.com/microscaler/BRRTRouter.git#subdirectory=tooling"
```

Point Cursor / Claude Desktop at `~/.local/share/brrtrouter/venv/bin/brrtrouter` (or set `BRRTROUTER_VENV` and use `$BRRTROUTER_VENV/bin/brrtrouter`).

**Running the server:**

```bash
# stdio transport (for Claude Desktop / CLI integrations)
brrtrouter mcp serve

# SSE transport (for web-based clients)
brrtrouter mcp serve --transport sse --host 127.0.0.1 --port 8765
```

**Claude Desktop config** (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "brrtrouter": {
      "command": "brrtrouter",
      "args": ["mcp", "serve"]
    }
  }
}
```

**Tools exposed:**

| Tool | Description |
|------|-------------|
| `lint_spec` | Lint an OpenAPI YAML string for BRRTRouter conformance |
| `check_spec_conformance` | Comprehensive conformance check (errors, warnings, suggestions) |
| `validate_openapi_dir` | Validate all `openapi.yaml` files under a directory |
| `fix_operation_ids` | Convert operationIds to snake_case (dry-run by default) |
| `list_spec_operations` | List all operations in a spec file |
| `generate_project` | Run `brrtrouter-gen generate` to create a gen crate |
| `generate_stubs` | Run `brrtrouter-gen generate-stubs` to create an impl crate |
| `generate_bff` | Generate a merged BFF spec from a suite config |
| `inspect_generated_dir` | Summarise the contents of a generated (gen) crate |
| `inspect_impl_dir` | Show which impl handlers are user-owned vs stubs |

**Resources exposed:**

| URI | Description |
|-----|-------------|
| `brrtrouter://guide/openapi-spec` | Guide for writing BRRTRouter-conformant specs |
| `brrtrouter://guide/code-generation` | Gen/impl layout, regeneration, and consumer `client build` / cargo `-p` naming |
| `brrtrouter://guide/bff-pattern` | BFF (Backend for Frontend) setup guide |
| `brrtrouter://reference/extensions` | All BRRTRouter OpenAPI extensions reference |
| `brrtrouter://examples/openapi-spec` | Minimal conformant OpenAPI 3.1.0 example |

**Prompts exposed:**

| Prompt | Arguments | Description |
|--------|-----------|-------------|
| `write_openapi_spec` | `service_name`, `description` | Prime assistant to write a conformant spec |
| `setup_bff` | `system_name`, `services` (comma-sep) | Prime assistant to create a BFF config |
| `implement_handler` | `operation_id`, `request_type`, `response_type` | Prime assistant to implement a handler stub |
| `review_spec` | `spec_content` | Prime assistant to review and improve a spec |

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
