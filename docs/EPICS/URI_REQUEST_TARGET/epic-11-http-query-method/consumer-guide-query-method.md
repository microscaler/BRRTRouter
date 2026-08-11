# Consumer guide — HTTP QUERY (RFC 10008)

When to use **GET** vs **QUERY** vs **POST**, how to call BRRTRouter, and what
to do when an edge returns **405**.

## GET vs QUERY vs POST

| Need | Prefer | Why |
|------|--------|-----|
| Simple filters that fit a short URI | **GET** + query string | Cacheable, bookmarkable; **must** follow [Epic 10](../BUILD_BOARD.md) percent-encoding (spaces → `%20`, not `+` in path) |
| Rich / large search body, still safe+idempotent | **QUERY** | Body carries the query; safe+idempotent ([retry policy](query-retry-policy.md)) |
| Non-idempotent mutation or write | **POST** (or PUT/PATCH) | Never use QUERY for side effects |

**Epic 10 is still required for GET query strings.** QUERY does not replace URI
parse/rebuild compliance.

## Declaring operations

See [declaring-query-operations.md](declaring-query-operations.md)
(`query:` or `x-brrtrouter-query`).

## Browser `fetch` (uppercase only)

Fetch does **not** case-normalize extension methods. Always use uppercase
`QUERY`:

```javascript
const res = await fetch("https://api.example/search", {
  method: "QUERY",
  headers: {
    "Content-Type": "application/json",
    Accept: "application/json",
  },
  body: JSON.stringify({ q: "South Africa", limit: 20 }),
});
```

Do **not** send a lowercase method token — it is distinct from `QUERY` and will
not match BRRTRouter QUERY routes (Story 11.1).

## CORS preflight

QUERY is **not** CORS-safelisted. Cross-origin browsers send OPTIONS first.
BRRTRouter permissive/default CORS include `QUERY` in
`Access-Control-Allow-Methods` (Story 11.1). Example preflight expectation:

```http
OPTIONS /search HTTP/1.1
Origin: https://app.example
Access-Control-Request-Method: QUERY
```

```http
HTTP/1.1 200 OK
Access-Control-Allow-Origin: https://app.example
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS, QUERY
```


## HTML forms

HTML `<form method="QUERY">` is **unsupported**. Browsers fall back to GET and
drop the body. Use `fetch` / XHR, or the POST fallback below.

## Caching

RFC 10008 allows cache keys that include the QUERY body (§2.7). **Browser and
CDN support is incomplete** as of 2026-08 — do not assume identical QUERY
requests are served from cache without hitting the origin.

## `Accept-Query`

Servers may advertise accepted QUERY body media types with `Accept-Query`
(RFC 10008 §3), e.g.:

```http
Accept-Query: application/json, application/x-www-form-urlencoded
```

BRRTRouter helpers: `brrtrouter::http::format_accept_query` /
`parse_accept_query` (`src/http/accept_query.rs`). Wire the header from your
handler or middleware when you want clients to discover media types.

## POST fallback (edges that 405)

Some CDNs/WAFs/envoys still reject QUERY with **405**. Until the edge is
updated, expose a documented **POST** twin that carries the same body:

1. Prefer a dedicated path such as `POST /search:query` **or**
2. Convention (document-only; not auto-routed by BRRTRouter today):

```http
POST /search HTTP/1.1
Content-Type: application/json
Query-Method: QUERY

{"q":"South Africa"}
```

Clients should:

1. Try `QUERY` first when the platform supports it.
2. On **405** (or known edge blocklist), retry with the POST fallback.
3. Never silently downgrade QUERY → GET (that drops the body).

OpenAPI: declare a separate `post` operation (or the twin path) alongside
`query:` — see the fixture `tests/fixtures/openapi_query_method.yaml`.

## Related

- Epic 11 README
- [query-retry-policy.md](query-retry-policy.md)
- Audit §5: `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md`
