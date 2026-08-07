# Epic 11 — HTTP QUERY method (RFC 10008)

**GitHub issue:** _(create and link)_  
**Theme labels:** `uri-request-target`, `rfc10008`, `epic`

## Overview

Add first-class support for the HTTP **QUERY** method ([RFC 10008](https://www.rfc-editor.org/info/rfc10008)):
safe + idempotent requests that carry a query in the **body**, with CORS and
OpenAPI integration. This epic is for rich/large searches that should not be
stuffed into GET query strings.

**Does not replace Epic 10.** URI parse/rebuild compliance remains mandatory for
all GET/path/query traffic. QUERY is an additional method, not a shortcut around
percent-encoding.

## Browser / ecosystem constraints (track in stories)

- `fetch({ method: "QUERY" })` works if **uppercase**; HTML forms do not.
- Not CORS-safelisted → preflight required.
- Browser HTTP cache for QUERY body-inclusive keys not widely implemented yet.
- Edges may 405 — document POST fallback.

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 11.1 | Method + router + CORS | [story-11.1-method-router-cors.md](story-11.1-method-router-cors.md) |
| 11.2 | OpenAPI QUERY operations | [story-11.2-openapi-query-operations.md](story-11.2-openapi-query-operations.md) |
| 11.3 | Proxy & client QUERY | [story-11.3-proxy-and-client-query.md](story-11.3-proxy-and-client-query.md) |
| 11.4 | Accept-Query + POST fallback docs | [story-11.4-accept-query-and-post-fallback.md](story-11.4-accept-query-and-post-fallback.md) |

## References

- RFC 10008
- `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` §5
- Epic 10 (prerequisite for correct request-target handling on all methods)
