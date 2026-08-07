# Epic 11 — HTTP QUERY method (RFC 10008)

**GitHub issue:** [#374](https://github.com/microscaler/BRRTRouter/issues/374)  
**Theme labels:** `uri-request-target`, `rfc10008`, `epic`

## Overview

Add first-class support for the HTTP **QUERY** method ([RFC 10008](https://www.rfc-editor.org/info/rfc10008)):
safe + idempotent requests that carry a query in the **body**, with CORS and
OpenAPI integration. This epic is for rich/large searches that should not be
stuffed into GET query strings.

**Does not replace Epic 10.** URI parse/rebuild compliance remains mandatory for
all GET/path/query traffic. QUERY is an additional method, not a shortcut around
percent-encoding.

## Success criteria (epic-level)

- [ ] Stories 11.1–11.4 meet [`TESTING_STANDARD.md`](../TESTING_STANDARD.md):
  comprehensive **positive and negative** unit tests (see each story’s Unit tests section).
- [ ] QUERY routing + CORS covered with allow and reject paths.
- [ ] Proxy/client QUERY does not regress Epic 10 composition error taxonomy.

## Browser / ecosystem constraints (track in stories)

- `fetch({ method: "QUERY" })` works if **uppercase**; HTML forms do not.
- Not CORS-safelisted → preflight required.
- Browser HTTP cache for QUERY body-inclusive keys not widely implemented yet.
- Edges may 405 — document POST fallback.

## Stories

| Story | Title | Issue | Doc |
|-------|--------|-------|-----|
| 11.1 | Method + router + CORS | [#386](https://github.com/microscaler/BRRTRouter/issues/386) | [story-11.1-…](story-11.1-method-router-cors.md) |
| 11.2 | OpenAPI QUERY operations | [#387](https://github.com/microscaler/BRRTRouter/issues/387) | [story-11.2-…](story-11.2-openapi-query-operations.md) |
| 11.3 | Proxy & client QUERY | [#388](https://github.com/microscaler/BRRTRouter/issues/388) | [story-11.3-…](story-11.3-proxy-and-client-query.md) |
| 11.4 | Accept-Query + POST fallback docs | [#389](https://github.com/microscaler/BRRTRouter/issues/389) | [story-11.4-…](story-11.4-accept-query-and-post-fallback.md) |

**Build:** see [BUILD_BOARD.md](../BUILD_BOARD.md) (start after Epic 10.1; 11.3 after 10.7).

## References

- RFC 10008
- `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` §5
- Epic 10 (prerequisite for correct request-target handling on all methods)
