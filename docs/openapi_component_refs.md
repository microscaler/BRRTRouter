# OpenAPI component `$ref` support (Story 12.3)

BRRTRouter resolves **local** component references while building routes.
Unsupported or dangling refs **fail closed** (load/`build_routes` returns an error)
instead of silently dropping schemas or routes.

## Supported

| Reference | Effect |
|-----------|--------|
| `#/components/schemas/*` | Schema expand (existing) |
| `#/components/parameters/*` | Parameter expand (existing) |
| `#/components/requestBodies/*` | Operation `requestBody: $ref` → schema + content types |
| `#/components/responses/*` | Response `$ref` → schema/example map |
| Path Item `$ref` → `#/components/pathItems/*` | Registers operations from the resolved path item |

Nested schema `$ref`s inside resolved bodies are expanded (`expand_schema_refs`,
cycle-safe).

## Unsupported (clear error)

- External HTTP(S) `$ref` targets
- Dangling / wrong-type component refs
- Circular pathItem `$ref` chains beyond depth 8

## Related

- Story: [`EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/story-12.3-openapi-ref-requestbodies-responses.md`](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/story-12.3-openapi-ref-requestbodies-responses.md)
- Version policy: [`OPENAPI_VERSION_SUPPORT.md`](OPENAPI_VERSION_SUPPORT.md)
