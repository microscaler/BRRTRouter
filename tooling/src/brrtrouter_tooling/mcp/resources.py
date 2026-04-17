"""Static reference content exposed as MCP resources.

Each function returns a string that is registered as a BRRTRouter MCP resource
and surfaced to AI assistants when they need context about BRRTRouter conventions.
"""

from __future__ import annotations

OPENAPI_SPEC_GUIDE = """\
# Writing OpenAPI Specs for BRRTRouter

## Required fields
- `openapi: 3.1.0`  (BRRTRouter only supports 3.1.0)
- `info.title`, `info.version`
- Every operation **must** have a unique `operationId` in **snake_case**
  (e.g. `list_pets`, `get_user_by_id`).  camelCase or kebab-case will fail the linter.

## operationId rules
- Must be non-empty
- First character: lowercase letter or underscore
- All characters: lowercase letters, digits, or underscores
- Use the linter to verify: `brrtrouter-gen lint --spec <path> --fail-on-error`

## Response schemas
- Always define typed schemas in `components/schemas` and `$ref` them from operations.
- Error responses should use `application/problem+json` with the `ProblemDetails` schema
  (RFC 7807 format: `type`, `title`, `status`, `detail`, `instance`).
- Use `type: array` with `items` for list responses; avoid anonymous inline arrays.

## Parameter conventions
- Define reusable parameters under `components/parameters` and `$ref` them.
- Standard pagination parameters: `LimitParam` (max 100) and `OffsetParam`.
- Path parameters are declared in the path template and in the operation `parameters` list.
- Array query parameters use `style: form, explode: false` (pipe-separated, e.g. `tags=a|b`).

## Request bodies
- JSON: `content: application/json` with a `$ref` schema.
- Form: `content: application/x-www-form-urlencoded`.
- File upload: `content: multipart/form-data`.

## Security schemes
Declare schemes under `components/securitySchemes`, then apply per-operation with `security:`.
Supported types:
- API Key (`in: header`, `in: query`, `in: cookie`)
- Bearer JWT (`scheme: bearer`)
- OAuth2 (`flows: clientCredentials` / `implicit` / etc.)

## BRRTRouter-specific OpenAPI extensions

### `x-sse: true`
Mark a GET endpoint as a Server-Sent Events stream.  The generator produces a
streaming handler instead of a normal request/response handler.
```yaml
/events:
  get:
    operationId: stream_events
    x-sse: true
    responses:
      "200":
        description: SSE stream
```

### `x-cors` (per-operation CORS override)
Controls CORS for a specific route.  Accepted values:
- `inherit` (default) — use global config from `config.yaml`
- `false` — disable CORS for this route
- object with optional keys: `allowedHeaders`, `allowedMethods`, `allowCredentials`,
  `exposeHeaders`, `maxAge`
```yaml
/public-data:
  get:
    operationId: get_public_data
    x-cors: inherit
```

### `x-brrtrouter-cors` (info-level global CORS hint)
Placed at the root `info` level.  Documents where CORS origins are loaded from at runtime:
```yaml
info:
  title: My API
  x-brrtrouter-cors:
    originsFromConfig: config/config.yaml
    devOriginExample: "http://localhost:3000"
```

### BFF proxy extensions (set automatically by `brrtrouter bff generate`)
- `x-brrtrouter-downstream-path`: full downstream URL path (`base_path + path`)
- `x-service`: name of the sub-service that owns this operation
- `x-service-base-path`: base path prefix for that service

## Minimal conformant spec example
```yaml
openapi: 3.1.0
info:
  title: My Service
  version: "1.0.0"

paths:
  /items:
    get:
      operationId: list_items
      summary: List all items
      parameters:
        - name: limit
          in: query
          schema:
            type: integer
            maximum: 100
      responses:
        "200":
          description: List of items
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: "#/components/schemas/Item"
        "400":
          description: Bad request
          content:
            application/problem+json:
              schema:
                $ref: "#/components/schemas/ProblemDetails"

components:
  schemas:
    Item:
      type: object
      required: [id, name]
      properties:
        id:
          type: integer
        name:
          type: string
    ProblemDetails:
      type: object
      properties:
        type:
          type: string
        title:
          type: string
        status:
          type: integer
        detail:
          type: string
        instance:
          type: string
```
"""

CODE_GENERATION_GUIDE = """\
# BRRTRouter Code Generation Guide

## Overview
`brrtrouter-gen` reads an OpenAPI 3.1.0 spec and generates two Rust crates:
1. **gen crate** — auto-generated; never edit directly
2. **impl crate** — user-owned handler bodies; safe to edit

## Generating the gen crate
```bash
brrtrouter-gen generate \\
  --spec path/to/openapi.yaml \\
  --output path/to/my_service_gen \\
  --force
```
Or via the Python tooling wrapper:
```bash
brrtrouter gen generate \\
  --spec path/to/openapi.yaml \\
  --output path/to/my_service_gen \\
  --project-root .
```

### What gets generated
```
my_service_gen/
├── Cargo.toml                  # workspace member manifest
├── src/
│   ├── main.rs                 # server entry point (registers handlers, starts server)
│   ├── lib.rs                  # crate root — re-exports handlers and controllers
│   ├── handlers/
│   │   ├── mod.rs              # handler module declarations
│   │   ├── types.rs            # all Request/Response structs (from schemas)
│   │   ├── list_items.rs       # per-operation request/response types
│   │   └── ...
│   └── controllers/
│       ├── mod.rs              # controller module declarations
│       ├── list_items.rs       # coroutine wrapper: receives raw request → typed handler
│       └── ...
├── config/
│   └── config.yaml             # runtime config (CORS origins, security, etc.)
└── doc/
    ├── openapi.yaml            # copy of the spec
    └── index.html              # Swagger UI
```

### Regenerating selectively
```bash
brrtrouter-gen generate --spec openapi.yaml --output my_gen --force \\
  --only handlers,controllers   # regenerate only specific parts
```
Supported parts: `handlers`, `controllers`, `types`, `registry`, `main`, `docs`

## Generating the impl crate (handler stubs)
```bash
brrtrouter-gen generate-stubs \\
  --spec path/to/openapi.yaml \\
  --output path/to/my_service_impl \\
  --component-name my_service_gen
```
Or via Python tooling:
```bash
brrtrouter gen generate-stubs \\
  --spec path/to/openapi.yaml \\
  --output path/to/my_service_impl \\
  --component-name my_service_gen \\
  --project-root .
```

### What stubs look like
Each handler stub is in `my_service_impl/src/handlers/<operation_id>.rs`:
```rust
// BRRTROUTER_USER_OWNED — generator will not overwrite this file
use my_service_gen::handlers::list_items::{ListItemsRequest, ListItemsResponse};

pub fn list_items(req: ListItemsRequest) -> ListItemsResponse {
    // TODO: implement
    ListItemsResponse::default()
}
```
The `// BRRTROUTER_USER_OWNED` sentinel prevents the generator from overwriting
your implementation when `--sync` is used.

### Syncing stubs after spec changes
When you add/change operations in the OpenAPI spec and regenerate the gen crate,
update only the stub *signature* without touching the body:
```bash
brrtrouter-gen generate-stubs \\
  --spec openapi.yaml --output my_impl --component-name my_gen --sync
```

## Linting the spec before generating
```bash
brrtrouter-gen lint --spec openapi.yaml --fail-on-error
```
Checks:
- All operationIds are snake_case
- Schema format consistency
- No missing `$ref` targets
- Missing or duplicate operationIds

## Dependency configuration
Place a `brrtrouter-dependencies.toml` next to your spec to pin extra
Cargo dependencies for the generated crate:
```toml
[dependencies]
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
```
Pass it with `--dependencies-config brrtrouter-dependencies.toml`.

## Workspace layout (recommended)
```
my-project/
├── openapi/
│   └── my_service/
│       └── openapi.yaml         # source of truth
├── my_service_gen/              # auto-generated (never edit)
│   └── src/
├── my_service_impl/             # user-owned handler implementations
│   └── src/
└── Cargo.toml                   # workspace [members]
```
"""

CONSUMER_CLI_BUILD_SECTION = """\

## Consumer CLI: host-aware build (`brrtrouter client build`)

Use this from consumer repos (e.g. PriceWhisperer) to compile a single impl crate with the
right `cargo -p` package name.

**Invocation:** `brrtrouter client build <system>_<module> [arch] [--workspace-dir microservices]`

**Default Cargo package (`-p`) for the impl crate:**
- **Standard services** (snake/kebab module names): `{snake_case(module)}_service_api_impl`
  Example: target `trader_amd` → package `amd_service_api_impl`.
- **BFF / camelCase module names** (any uppercase in the module segment): `{module}_impl`
  Example: target `bff_traderBFF` → package `traderBFF_impl` (matches typical `impl/Cargo.toml`).

**Optional `--package`:**
- Omit it when the defaults above match your workspace (recommended in Tiltfile loops).
- Shorthand `foo_impl` expands to `foo_service_api_impl` when `foo` is all lowercase.
- Full names ending with `_service_api_impl` are used as-is.
- Names starting with `rerp_` are passed through (legacy RERP workspaces).

**Impl crate `Cargo.toml`:** generated `main.rs` uses `clap` (CLI) and `may` (stack size). Those
dependencies must appear in the impl manifest (`clap` / `may` with `workspace = true` when using a
workspace). Re-run the tooling fixer or regenerate stubs if builds fail with unresolved `clap`/`may`.

**Examples:**
```bash
brrtrouter client build trader_amd
brrtrouter client build trader_market-data
brrtrouter client build bff_traderBFF
brrtrouter client build trader_amd --package amd_impl   # optional; same as default
```
"""

CODE_GENERATION_GUIDE = CODE_GENERATION_GUIDE.rstrip() + "\n" + CONSUMER_CLI_BUILD_SECTION

BFF_PATTERN_GUIDE = """\
# BFF (Backend for Frontend) Pattern with BRRTRouter

## What is a BFF?
A Backend for Frontend is an API layer that sits between clients and downstream
microservices.  It aggregates, adapts, and optimises responses for specific
client types (mobile, web, partner), enforces consistent error handling, and
applies client-specific security policies.

## Architecture
```
Clients (Mobile, Web, Partners)
        │
        ▼
   BFF (BRRTRouter)         ← single OpenAPI contract per client type
        │        │
        ▼        ▼
  Service A   Service B     ← each is itself a BRRTRouter service
```

## BFF spec generation with brrtrouter tooling

### Suite config YAML
Define a `bff-suite-config.yaml` that lists each downstream service:
```yaml
openapi_base_dir: openapi          # relative to --base-dir (default: cwd)
output_path: openapi/bff/openapi.yaml

metadata:
  title: My BFF API
  version: "1.0.0"
  description: Aggregated BFF for web client
  security_schemes:
    BearerAuth:
      type: http
      scheme: bearer
  security:
    - BearerAuth: []

services:
  users:
    base_path: /api/users
    spec_path: users/openapi.yaml
  orders:
    base_path: /api/orders
    spec_path: orders/openapi.yaml
```

### Generate the merged BFF spec
```bash
brrtrouter bff generate \\
  --suite-config bff-suite-config.yaml \\
  --validate
```

### Directory-based discovery (no config file needed)
If your openapi dir follows `openapi/{system}/{service}/openapi.yaml`:
```bash
brrtrouter bff generate-system \\
  --openapi-dir openapi \\
  --system my_system
```

### Tiltfile: BFF spec regeneration (`bff-spec-gen`) — do not hardcode `deps`

`brrtrouter bff generate-system` merges every sub-service under `openapi_dir/{system}/`.
The Tilt `local_resource` that runs this command **must not** list individual
`.../openapi.yaml` paths by hand in `deps`: new services would be skipped and Tilt would
not re-run the merge when those specs change.

**Use the same service list as your microservice loops**, from `brrtrouter client tilt scan`
(so ports, binary names, and BFF inputs stay aligned):

```python
services_json = str(
    local(
        '~/.local/share/brrtrouter/venv/bin/brrtrouter client tilt scan --dir microservices/openapi/trader --base-port 8002',
        quiet=True,
    )
).strip()
tilt_config = decode_json(services_json)
TRADER_SERVICES = tilt_config['services']

local_resource(
    'bff-spec-gen',
    cmd='''
        ~/.local/share/brrtrouter/venv/bin/brrtrouter client bff generate-system \\
            --system trader \\
            --output openapi/bff/openapi_bff.yaml
    ''',
    deps=[
        './microservices/openapi/trader/%s/openapi.yaml' % name for name in TRADER_SERVICES
    ] + ['./tooling/pyproject.toml'],
    ignore=['./microservices/openapi/bff/openapi_bff.yaml'],
)
```

**Factor helpers:** put repeated `local_resource` definitions in `./tilt/lib.tilt` and `load()` them from the root `Tiltfile` (Starlark only; still no Python import). See `brrtrouter://guide/tilt-setup` sections 3-4 for full examples.

Tune `--dir`, file paths, and `--base-port` for your repository. Default `--openapi-dir` is
`./openapi` (cwd-relative); a symlink `openapi` → `microservices/openapi` is common.

**If the merge is driven by `bff generate --suite-config`** (not `generate-system`), include
the suite config file (e.g. `openapi/bff/bff-suite-config.yaml`) in `deps`; `generate-system`
does not read that file.

## What the BFF generator does
1. Loads each sub-service spec from `spec_path`
2. Prefixes all paths with `base_path` (e.g. `/pets` → `/api/pets-service/pets`)
3. Prefixes all schema names with the service name (e.g. `Pet` → `PetsServicePet`)
4. Merges `components.parameters`, `components.securitySchemes`, root `security`
5. Adds proxy routing extensions to each operation:
   - `x-brrtrouter-downstream-path`: full downstream path (used by the proxy handler)
   - `x-service`: name of the owning sub-service
   - `x-service-base-path`: base path prefix for that service

## Implementing BFF aggregation in handlers
The generated BFF impl stubs are the same as any other impl crate.  Use the
typed request/response structs to call downstream services:
```rust
pub fn get_user_dashboard(req: GetUserDashboardRequest) -> GetUserDashboardResponse {
    let user = user_service::get_user(req.user_id)?;
    let orders = order_service::list_orders(req.user_id)?;
    GetUserDashboardResponse {
        user,
        recent_orders: orders,
    }
}
```

## BFF error handling
- All services should return `application/problem+json` (RFC 7807 Problem Details).
- The BFF can translate upstream errors or aggregate partial failures.
- Define a shared `ProblemDetails` schema in components and `$ref` it from every
  error response.

## Latency guidelines
- Same datacenter: 1-5 ms per service hop
- Cross-region: 10-50 ms per hop
- Keep service depth <= 2-3 levels to stay within latency budgets

## Port registry
Use `brrtrouter ports validate` to detect port conflicts across services:
```bash
brrtrouter ports validate --project-root .
```
"""

EXTENSIONS_REFERENCE = """\
# BRRTRouter OpenAPI Extension Reference

## Per-operation extensions

| Extension | Type | Description |
|-----------|------|-------------|
| `x-sse` | `boolean` | `true` → generate SSE streaming handler instead of request/response handler |
| `x-cors` | `string|object` | CORS override: `"inherit"` (default), `false`, or config object |

### `x-cors` object shape
```yaml
x-cors:
  allowedHeaders: ["Authorization", "Content-Type"]
  allowedMethods: ["GET", "POST"]
  allowCredentials: true
  exposeHeaders: ["X-Request-Id"]
  maxAge: 3600
```

## Info-level extensions

| Extension | Description |
|-----------|-------------|
| `x-brrtrouter-cors` | Documents CORS origin source; `originsFromConfig` and `devOriginExample` |

## BFF proxy extensions (auto-generated; do not write manually)

| Extension | Description |
|-----------|-------------|
| `x-brrtrouter-downstream-path` | Full downstream path the BFF should proxy to |
| `x-service` | Name of the owning sub-service |
| `x-service-base-path` | Base path prefix for the sub-service |

## Linter rules enforced by `brrtrouter-gen lint`

| Rule | Description |
|------|-------------|
| snake_case operationId | All operationIds must match `[a-z_][a-z0-9_]*` |
| No missing $ref | Every `$ref` must resolve within the spec |
| Unique operationIds | No two operations share an operationId |
| Schema completeness | All response/request schemas must have `type` |
| Decimal formats | `number` fields should have `format: decimal` or `format: money` |
"""

EXAMPLE_OPENAPI_YAML = """\
# Minimal BRRTRouter-conformant OpenAPI 3.1.0 spec
# Demonstrates: path params, query params, JSON body, security, SSE, error responses

openapi: 3.1.0
info:
  title: Example Service
  version: "1.0.0"
  description: Minimal example service spec for BRRTRouter
  x-brrtrouter-cors:
    originsFromConfig: config/config.yaml
    devOriginExample: "http://localhost:3000"

servers:
  - url: http://localhost:8081
    description: Local dev

paths:
  /items:
    get:
      operationId: list_items
      summary: List items with pagination
      x-cors: inherit
      parameters:
        - $ref: "#/components/parameters/LimitParam"
        - $ref: "#/components/parameters/OffsetParam"
      responses:
        "200":
          description: Paginated list of items
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: "#/components/schemas/Item"
        "400":
          description: Invalid request
          content:
            application/problem+json:
              schema:
                $ref: "#/components/schemas/ProblemDetails"

    post:
      operationId: create_item
      summary: Create a new item
      security:
        - BearerAuth: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/CreateItemRequest"
      responses:
        "201":
          description: Item created
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Item"
        "400":
          description: Validation error
          content:
            application/problem+json:
              schema:
                $ref: "#/components/schemas/ProblemDetails"
        "401":
          description: Unauthorized
          content:
            application/problem+json:
              schema:
                $ref: "#/components/schemas/ProblemDetails"

  /items/{id}:
    get:
      operationId: get_item
      summary: Get a single item
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: Item found
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Item"
        "404":
          description: Not found
          content:
            application/problem+json:
              schema:
                $ref: "#/components/schemas/ProblemDetails"

  /events:
    get:
      operationId: stream_events
      summary: Server-Sent Events stream
      x-sse: true
      responses:
        "200":
          description: SSE stream of events

components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer

  parameters:
    LimitParam:
      name: limit
      in: query
      description: Maximum number of results
      schema:
        type: integer
        default: 20
        minimum: 1
        maximum: 100

    OffsetParam:
      name: offset
      in: query
      description: Pagination offset
      schema:
        type: integer
        default: 0
        minimum: 0

  schemas:
    Item:
      type: object
      required: [id, name]
      properties:
        id:
          type: string
        name:
          type: string
        description:
          type: string

    CreateItemRequest:
      type: object
      required: [name]
      properties:
        name:
          type: string
        description:
          type: string

    ProblemDetails:
      type: object
      properties:
        type:
          type: string
        title:
          type: string
        status:
          type: integer
        detail:
          type: string
        instance:
          type: string
"""


def get_openapi_spec_guide() -> str:
    """Return the BRRTRouter OpenAPI spec authoring guide."""
    return OPENAPI_SPEC_GUIDE


def get_code_generation_guide() -> str:
    """Return the brrtrouter-gen code generation guide."""
    return CODE_GENERATION_GUIDE


def get_bff_pattern_guide() -> str:
    """Return the BFF (Backend for Frontend) pattern guide."""
    return BFF_PATTERN_GUIDE


def get_extensions_reference() -> str:
    """Return the BRRTRouter OpenAPI extension reference."""
    return EXTENSIONS_REFERENCE


def get_example_openapi_yaml() -> str:
    """Return a minimal conformant OpenAPI 3.1.0 example spec."""
    return EXAMPLE_OPENAPI_YAML


TILT_SETUP_GUIDE = """\
# Tilt Setup Guide for BRRTRouter Infrastructures

## Overview
BRRTRouter encourages using [Tilt](https://tilt.dev/) for local environment orchestration.
This document outlines the standard loop-based architecture for deploying microservices
efficiently without duplicating `Dockerfile` and Tilt resource definitions.

## 0. Shared Python environment (`brrtrouter` CLI)
Use **one** virtualenv on your machine for the `brrtrouter` CLI (and, in Hauliage, the `hauliage` CLI installed into the same env):

- **Default path:** `~/.local/share/brrtrouter/venv`
- **Override:** set `BRRTROUTER_VENV` to the venv directory (the one that contains `bin/brrtrouter`).

Create and install (from a BRRTRouter clone):

```bash
python3 -m venv ~/.local/share/brrtrouter/venv
~/.local/share/brrtrouter/venv/bin/pip install -U pip
~/.local/share/brrtrouter/venv/bin/pip install -e ./tooling[dev]   # or tooling[mcp] for MCP
```

Consumer Tiltfiles (PriceWhisperer, Hauliage) resolve `brrtrouter_bin` / `hauliage_bin` from `BRRTROUTER_VENV` with this default. Per-repo `tooling/.venv` is **not** required for Tilt.

## 1. Unified Docker templating
Instead of maintaining individual `Dockerfile.<service>` files, use a unified `Dockerfile.template`
that dynamically injects configurations at build-time. We use `brrtrouter client docker build-image-simple`
to render and build this template.

Your `docker/microservices/Dockerfile.template` should look like this:
```dockerfile
FROM alpine:3.19

ARG BINARY_NAME
ARG SYSTEM_NAME
ARG MODULE_NAME
ARG TARGET_PORT
ARG USERNAME=appuser
ARG USER_UID=1000
ARG USER_GID=1000

# Install dependencies like ca-certificates
RUN apk update && apk add --no-cache ca-certificates tzdata sqlite

# Create a non-root user
RUN addgroup -g $USER_GID -S $USERNAME && adduser -u $USER_UID -S -G $USERNAME $USERNAME

WORKDIR /app

# The compiled musl binary will be injected here during build
COPY $BINARY_NAME /app/$BINARY_NAME

# Ensure executable permissions
RUN chmod +x /app/$BINARY_NAME && chown -R $USERNAME:$USERNAME /app

# Switch to non-root user
USER $USERNAME

# Expose the dynamically mapped port
EXPOSE $TARGET_PORT

# Entrypoint needs to shell-evaluate $BINARY_NAME
ENTRYPOINT ["sh", "-c", "exec /app/$BINARY_NAME"]
```

## 2. Tiltfile Microservice Loop
Instead of defining manual `custom_build` and `local_resource` blocks for every microservice,
define a standard array of services and loop over them.

```python
# List of services to deploy
MICROSERVICES = [
    'service1',
    'service2',
    'service3'
]

# Map service names to their dynamically assigned ports
SERVICE_PORTS = {
    'service1': 8001,
    'service2': 8002,
    'service3': 8003
}

# The single deployment function
def create_microservice_deployment(name):
    port = SERVICE_PORTS.get(name, 8080)
    binary_name = f'system_module_svc_{name}'
    target_path = f'microservices/target/x86_64-unknown-linux-musl/debug/module_{name}_impl'
    artifact_path = f'build_artifacts/{binary_name}'
    image_name = f'localhost:5001/system-{name}'
    hash_path = f'build_artifacts/{binary_name}.sha256'

    # 1. Build binary (impl crate -p is derived by the CLI, e.g. trader_foo -> foo_service_api_impl)
    local_resource(
        f'build-{name}',
        f'~/.local/share/brrtrouter/venv/bin/brrtrouter client build mysys_{name}',
        ...
    )

    # 2. Copy binary
    local_resource(
        f'copy-{name}',
        f'~/.local/share/brrtrouter/venv/bin/brrtrouter client docker copy-binary {target_path} {artifact_path} {binary_name}',
        resource_deps=[f'build-{name}']
    )

    # 3. Build Image
    local_resource(
        f'docker-{name}',
        f'~/.local/share/brrtrouter/venv/bin/brrtrouter client docker build-image-simple {image_name} {hash_path} {artifact_path} --system mysys --module mymod --port {port} --binary-name {binary_name}',
        resource_deps=[f'copy-{name}']
    )

    # 4. Custom build (for Tilt hot-reloading)
    custom_build(
        image_name,
        f'(docker image inspect {image_name}:tilt >/dev/null 2>&1) || ~/.local/share/brrtrouter/venv/bin/brrtrouter client docker build-image-simple {image_name} {image_name} {hash_path} {artifact_path} --system mysys --module mymod --port {port} --binary-name {binary_name} && (docker push {image_name}:tilt || kind load docker-image {image_name}:tilt --name mycluster)',
        deps=[artifact_path, hash_path],
        tag='tilt',
        live_update=[
            sync(artifact_path, f'/app/{binary_name}'),
            run('kill -HUP 1', trigger=[artifact_path]),
        ]
    )

# Instantiate loop
for service in MICROSERVICES:
    create_microservice_deployment(service)
```

## 3. Starlark vs Python (BRRTRouter is not importable from Tilt)
Tiltfiles are **Starlark**, not Python. You **cannot** `import brrtrouter_tooling` or call Python APIs from a `Tiltfile` or from `./tilt/lib.tilt`.

**Correct boundary:** use the **`brrtrouter` CLI** inside `local_resource` / `local()` command strings, e.g.:
`~/.local/share/brrtrouter/venv/bin/brrtrouter client gen suite ...`

**What you can import in Starlark:** other **Tilt/Starlark** files via `load()` / `load_dynamic()`, or Tilt **extensions** via `load('ext://name', ...)`. Those files only contain Starlark—shared helpers, constants, and wrappers that still call `brrtrouter` through the shell.

**Anti-pattern:** expecting `./tilt/lib.tilt` to "import" BRRTRouter Python modules. That will never work; use CLI instead.

## 4. Example: `./tilt/lib.tilt` for shared Starlark helpers
Keep the **root `Tiltfile` thin**: `load()` one library file, then loops and `config.parse()`. Put repeated `local_resource` patterns in **`tilt/lib.tilt`** (or `tilt/lib.tilt` + `tilt/bff.tilt` if you split further).

### `tilt/lib.tilt` (Starlark library — example)
```python
# tilt/lib.tilt — Starlark only. Cannot import Python or brrtrouter_tooling.

def brrtrouter_bin():
    v = os.getenv("BRRTROUTER_VENV", "").strip().rstrip("/")
    if v:
        return v + "/bin/brrtrouter"
    h = os.getenv("HOME", "") or os.getenv("USERPROFILE", "")
    return h + "/.local/share/brrtrouter/venv/bin/brrtrouter"


def create_trader_service_gen(name, openapi_spec_relpath):
    # Register codegen for one trader service (example paths for a typical layout).
    cmd = "%s client gen suite trader --service %s --openapi-dir microservices/openapi" % (
        brrtrouter_bin(),
        name,
    )
    local_resource(
        "%s-service-gen" % name,
        cmd=cmd,
        deps=[
            "./microservices/openapi/trader/%s" % openapi_spec_relpath,
            "./tooling/pyproject.toml",
        ],
        resource_deps=["%s-lint" % name],
        labels=["trd_" + name],
        allow_parallel=True,
    )


def create_bff_spec_gen_deps(trader_service_names):
    # deps for bff-spec-gen: every trader openapi.yaml + tooling (see bff-pattern guide).
    paths = ["./microservices/openapi/trader/%s/openapi.yaml" % n for n in trader_service_names]
    return paths + ["./tooling/pyproject.toml"]
```

### Root `Tiltfile` (loads library, runs scan, loops)
```python
# Tiltfile at repo root
load(
    "./tilt/lib.tilt",
    "brrtrouter_bin",
    "create_trader_service_gen",
    "create_bff_spec_gen_deps",
)

services_json = str(
    local(
        brrtrouter_bin() + " client tilt scan --dir microservices/openapi/trader --base-port 8002",
        quiet=True,
    )
).strip()
tilt_config = decode_json(services_json)
TRADER_SERVICES = tilt_config["services"]

for name in TRADER_SERVICES:
    create_trader_service_gen(name, "%s/openapi.yaml" % name)

local_resource(
    "bff-spec-gen",
    cmd="set -e && " + brrtrouter_bin() + " client bff generate-system --system trader --output openapi/bff/openapi_bff.yaml",
    deps=create_bff_spec_gen_deps(TRADER_SERVICES),
    ignore=["./microservices/openapi/bff/openapi_bff.yaml"],
    labels=["bff"],
)
```

Paths and flags must match your repo (`--openapi-dir`, `--output`, symlink `openapi` → `microservices/openapi`, etc.). The library file **only** reduces duplication; **BRRTRouter behavior always runs through the CLI** in `cmd=`.

## 5. brrtrouter local tools support
The `brrtrouter` toolset includes several subcommands designed for this model:
- `brrtrouter client build <system>_<module>`: Host-aware `cargo`/`cargo zigbuild` for one impl crate.
  Default `-p` is `{snake}_service_api_impl` for normal modules, or `{Module}_impl` when the module
  segment is camelCase (e.g. `bff_traderBFF` → `traderBFF_impl`). Prefer omitting `--package` in Tiltfile loops.
- `brrtrouter client docker build-image-simple`: Renders the `Dockerfile.template` filling in variables and pushing images caching mechanisms.
- `brrtrouter client docker copy-binary`: Efficient binary copying using hashes preventing unaffected binaries from re-triggering container pushes.
- `brrtrouter client tilt scan`: Emits JSON with `services` (and ports, binary names) for a tree under `microservices/openapi/<system>/`. Use that list for **all** Tilt loops *and* for the `deps` of any `local_resource` that runs `bff generate-system`, so BFF regeneration tracks every trader spec automatically. See `brrtrouter://guide/bff-pattern` (Tiltfile `bff-spec-gen` section).

## 6. Running
Just use `tilt up`. Tilt will now concurrently parse and loop over `MICROSERVICES` and create parallel pipelines that don't crowd the config file.
"""


def get_tilt_setup_guide() -> str:
    """Return the Tilt configuration guide for setting up BRRTRouter loop configurations."""
    return TILT_SETUP_GUIDE
