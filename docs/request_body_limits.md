# Inbound request body limits (Story 12.2)

BRRTRouter enforces **hard caps** on inbound request bodies and returns
**413 Payload Too Large** before handler dispatch.

## Knobs

| Source | Effect |
|--------|--------|
| `BRRTROUTER_MAX_REQUEST_BODY_OCTETS` | Global ceiling (decimal octets). Default **16 MiB**. `0` / invalid → default. |
| `RouteMeta.estimated_request_body_bytes` | Per-route ceiling from schema heuristic and/or vendor override. Effective limit = `min(global, estimate)` when set. |
| `x-brrtrouter-body-size-bytes` on the request schema | Vendor override for the estimate (can raise above the heuristic; still capped by the estimate max and the global env). |

## Behavior

1. If `Content-Length` is present and exceeds the **global** max (or is non-decimal / hostile), reject **413** without reading the body.
2. Otherwise read with a stream cap (`global + 1`); overrun → **413** (no silent truncate).
3. After route match, if measured body (`Content-Length` or octets read) exceeds the **effective route** limit → **413**.

### Error JSON

```json
{
  "error": "Payload Too Large",
  "reason": "request_body_too_large",
  "message": "…"
}
```

## Related

- Epic story: [`EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/story-12.2-hard-inbound-body-limits.md`](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/story-12.2-hard-inbound-body-limits.md)
- Extension wiki: [`../llmwiki/reference/openapi-extensions.md`](../llmwiki/reference/openapi-extensions.md)
- Stack sizing (separate): [`stack_size.md`](stack_size.md)
