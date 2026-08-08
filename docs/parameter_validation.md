# Pre-handler parameter validation (Story 12.4)

After route match and **authentication**, BRRTRouter validates OpenAPI parameters
declared on the route (`RouteMeta.parameters`) **before** the handler runs.

## Enforced today

| Check | Behaviour |
|-------|-----------|
| Required path / query / header / cookie missing | **400** |
| Required present but empty/whitespace | **400** |
| Schema `type: integer\|number\|boolean` with non-parseable value | **400** (no silent string coerce) |
| Single value longer than 8192 octets | **400** `value_too_large` |

Unknown query keys are **not** rejected (`additionalProperties` not enforced yet).

## Error JSON

```json
{
  "error": "Bad Request",
  "reason": "parameter_validation_failed",
  "message": "One or more request parameters are missing or invalid",
  "fields": [{ "name": "q", "in": "query", "error": "required" }]
}
```

## Order

1. Route match  
2. Security / auth (401/403 first — Story 12.4 P6)  
3. **Parameter validation**  
4. Content-Type / body schema validation  
5. Handler dispatch  

Untyped and proxy routes still validate when `parameters` is non-empty.

## Related

- Epic story: [`EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/story-12.4-pre-handler-param-validation.md`](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/story-12.4-pre-handler-param-validation.md)
- Body limits: [`request_body_limits.md`](request_body_limits.md)
