# Declaring QUERY operations in OpenAPI (Story 11.2)

Until OpenAPI / `oas3` expose a native path-item `query` field everywhere,
BRRTRouter accepts RFC 10008 **QUERY** operations via one of:

1. **Preferred for suite configs:** path-level `query:` (case-insensitive) — the
   loader rewrites this to `x-brrtrouter-query` before deserialize.
2. **Explicit extension:** `x-brrtrouter-query:` with a full Operation Object
   (including `requestBody`).

Do **not** rely on tooling that silently strips unknown verbs: a bare `query:`
key is dropped by `oas3` 0.21 unless this loader promotes it first. Loading
through `brrtrouter::load_spec` / `load_spec_full` always promotes.

## Example

```yaml
openapi: 3.1.0
info:
  title: QUERY demo
  version: 1.0.0
paths:
  /search:
    get:
      operationId: get_search
      responses:
        "200":
          description: ok
    query:
      operationId: query_search
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [q]
              properties:
                q:
                  type: string
          application/x-www-form-urlencoded:
            schema:
              type: object
              properties:
                q:
                  type: string
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                type: object
```

Equivalent extension form (same path item):

```yaml
x-brrtrouter-query:
  operationId: query_search
  requestBody:
    required: true
    content:
      application/json:
        schema:
          type: object
          required: [q]
          properties:
            q: { type: string }
```

## Fail-closed rules

| Case | Result |
|------|--------|
| Both `query` and `x-brrtrouter-query` | load error (duplicate) |
| `query` is not an object | load error |
| Illegal path template (`/x/{`) | validation error |
| Missing `operationId` / `x-handler-*` | validation error (no half handler) |
| Other unknown verbs (`search:`) | still stripped (legacy); not QUERY |

JSON body schemas drive codegen/`request_schema`. Form media types are recorded
in `request_content_types` for 415 enforcement (same as POST).

Fixture used by unit tests:
`tests/fixtures/openapi_query_method.yaml`.
