# QUERY retry policy (Story 11.3)

RFC 10008 defines **QUERY** as both **safe** and **idempotent**. Automatic
retry / replay policies may therefore treat QUERY like GET/HEAD/OPTIONS.

Classifier: [`method_allows_automatic_retry`](../../../../src/http/method_ext.rs)
(`GET` | `HEAD` | `OPTIONS` | `TRACE` | `QUERY`).

| Method | Auto-retry? | Notes |
|--------|-------------|--------|
| QUERY | yes | Safe + idempotent (RFC 10008 §2) |
| GET / HEAD / OPTIONS | yes | Safe + idempotent |
| PUT / DELETE | no (this classifier) | Idempotent but not safe — callers may retry with explicit policy |
| POST / PATCH | no | Not idempotent |

Outbound client: [`fetch_query`](../../../../src/http/fetch.rs). BFF:
[`proxy_untyped`](../../../../src/http/proxy.rs) maps
`HandlerRequest.method` via `http` 0.2 `Method::from_bytes` (QUERY extension).
