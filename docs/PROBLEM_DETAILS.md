# Problem Details (RFC 7807 / RFC 9457) — Epic 13.3

Framework-generated client/server errors use
`Content-Type: application/problem+json` with stable `type` URIs under
`https://microscaler.dev/problems/…`.

Legacy keys `error`, `message`, and `reason` remain for one migration cycle.
Field-level validation also emits a `fields` array extension.

## Escape hatch

Set `BRRTR_LEGACY_ERROR_JSON=1` (or `true` / `yes`) to emit the pre-13.3
`application/json` shape without `type` / `status` members.

## Type catalog

| `type` URI | Typical status | `reason` extension |
|------------|----------------|--------------------|
| `…/parameter-validation-failed` | 400 | `parameter_validation_failed` |
| `…/request-body-too-large` | 413 | `request_body_too_large` |
| `…/multipart-missing-boundary` | 400 | `multipart_missing_boundary` |
| `…/multipart-malformed` | 400 | `multipart_malformed` |
| `…/multipart-file-too-large` | 413 | `multipart_file_too_large` |
| `…/unauthorized` | 401 | — |
| `…/forbidden` | 403 | — |
| `…/not-found` | 404 | — |
| `…/uri-too-long` | 414 | — |
| `…/bad-request` | 400 | — |
| `…/rate-limit-exceeded` | 429 | `rate_limit_exceeded` |
| `…/internal-error` | 5xx | — |

Prefix: `https://microscaler.dev/problems/`.

## Example (parameter validation)

```json
{
  "type": "https://microscaler.dev/problems/parameter-validation-failed",
  "title": "Bad Request",
  "status": 400,
  "detail": "One or more request parameters are missing or invalid",
  "reason": "parameter_validation_failed",
  "error": "One or more request parameters are missing or invalid",
  "message": "One or more request parameters are missing or invalid",
  "fields": [
    { "name": "limit", "in": "query", "error": "required" }
  ]
}
```

## API

- Builder: `brrtrouter::http::problem::Problem`
- Wire helper: `brrtrouter::http::problem::write_problem`
- `HandlerResponse::error` and `write_json_error` emit problem+json by default.
