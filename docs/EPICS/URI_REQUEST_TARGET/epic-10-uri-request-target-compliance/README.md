# Epic 10 — Request-target parse & rebuild (100% URI compliance)

**GitHub issue:** _(create and link)_  
**Theme labels:** `uri-request-target`, `epic`

## Overview

Make BRRTRouter’s handling of the HTTP **request target** (path + query)
fully compliant with:

- **RFC 3986** — URI syntax, percent-encoding, decode-for-process / encode-for-reconstitute (§2.4)
- **RFC 9110** — HTTP semantics, request target, length guidance
- **WHATWG `application/x-www-form-urlencoded`** — inbound query decode (browser-compatible)

Triggered by Loadlinker BFF 502s when rebuilt query strings contained raw
spaces (`country=South Africa`). Encoding was fixed; this epic closes every
remaining parse/rebuild gap so complex queries (accents, reserved chars,
multi-value OpenAPI styles, long targets) cannot regress into `InvalidUri`,
silent corruption, or mis-labelled 502s.

**Out of scope:** RFC 10008 HTTP QUERY method → [Epic 11](../epic-11-http-query-method/).

## Success criteria (epic-level)

- [ ] Compliance matrix in Story 10.1 is entirely **Pass** (automated).
- [ ] `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` scorecard updated; no open “Gap” rows for URI parse/rebuild.
- [ ] Inbound illegal inputs fail closed with correct status; legal Unicode/reserved inputs round-trip.
- [ ] Proxy never returns **502** for URI composition failures.
- [ ] Loadlinker-class geography names (spaces, diacritics) covered by golden + fuzz suites.

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 10.1 | Spec matrix & golden corpus | [story-10.1-spec-matrix-and-golden-corpus.md](story-10.1-spec-matrix-and-golden-corpus.md) |
| 10.2 | Inbound query parse edge cases | [story-10.2-inbound-query-parse-edge-cases.md](story-10.2-inbound-query-parse-edge-cases.md) |
| 10.3 | Inbound path segment decode | [story-10.3-inbound-path-segment-decode.md](story-10.3-inbound-path-segment-decode.md) |
| 10.4 | Component-specific encoders | [story-10.4-component-specific-encoders.md](story-10.4-component-specific-encoders.md) |
| 10.5 | Proxy path/query passthrough | [story-10.5-proxy-path-query-passthrough.md](story-10.5-proxy-path-query-passthrough.md) |
| 10.6 | Request-target length → 414 | [story-10.6-request-target-length-414.md](story-10.6-request-target-length-414.md) |
| 10.7 | Error taxonomy | [story-10.7-error-taxonomy.md](story-10.7-error-taxonomy.md) |
| 10.8 | Unify http URI stack | [story-10.8-unify-http-uri-stack.md](story-10.8-unify-http-uri-stack.md) |
| 10.9 | OpenAPI style/explode fidelity | [story-10.9-openapi-style-explode-fidelity.md](story-10.9-openapi-style-explode-fidelity.md) |
| 10.10 | Property/fuzz compliance suite | [story-10.10-property-fuzz-compliance-suite.md](story-10.10-property-fuzz-compliance-suite.md) |
| 10.11 | may_minihttp request-line boundary | [story-10.11-may-minihttp-request-line-boundary.md](story-10.11-may-minihttp-request-line-boundary.md) |

## Primary code surfaces

- `src/server/request.rs` — `parse_query_params`, `parse_request`, `decode_param_value`
- `src/http/proxy.rs` — `resolve_path_template`, `proxy_untyped`
- `src/router/` — path template matching / path params
- may_minihttp (sibling) — request-line → path string handed to BRRTRouter

## References

- Audit: `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md`
- Postmortem: `docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`
- RFC 3986, RFC 9110, WHATWG URL (`application/x-www-form-urlencoded`)
