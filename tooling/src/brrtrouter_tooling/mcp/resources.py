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
| `x-cors` | `string\\|object` | CORS override: `"inherit"` (default), `false`, or config object |

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
  - url: http://localhost:8080
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
