# Epic 10 — Request-target parse & rebuild (100% URI compliance)

**GitHub issue:** [#373](https://github.com/microscaler/BRRTRouter/issues/373)  
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

- [x] Compliance matrix in Story 10.1 is entirely **Pass** (automated) — `REQ-URI-VALIDATE` remains **Partial** (http 0.2 not a full RFC 3986 validator; dual-stack gated).
- [x] Scorecard: URI parse/rebuild Gap rows closed (see compliance-matrix.md).
- [x] Inbound illegal inputs fail closed with correct status; legal Unicode/reserved inputs round-trip.
- [x] Proxy never returns **502** for URI composition failures.
- [x] Loadlinker-class geography names (spaces, diacritics) covered by golden + property suites.
- [x] **Every story** meets [`TESTING_STANDARD.md`](../TESTING_STANDARD.md): comprehensive **positive and negative** unit tests (minima enforced in each story’s Unit tests section).

## Stories

| Story | Title | Issue | Doc |
|-------|--------|-------|-----|
| 10.1 | Spec matrix & golden corpus | [#375](https://github.com/microscaler/BRRTRouter/issues/375) | [story-10.1-…](story-10.1-spec-matrix-and-golden-corpus.md) |
| 10.2 | Inbound query parse edge cases | [#376](https://github.com/microscaler/BRRTRouter/issues/376) | [story-10.2-…](story-10.2-inbound-query-parse-edge-cases.md) |
| 10.3 | Inbound path segment decode | [#377](https://github.com/microscaler/BRRTRouter/issues/377) | [story-10.3-…](story-10.3-inbound-path-segment-decode.md) |
| 10.4 | Component-specific encoders | [#378](https://github.com/microscaler/BRRTRouter/issues/378) | [story-10.4-…](story-10.4-component-specific-encoders.md) |
| 10.5 | Proxy path/query passthrough | [#379](https://github.com/microscaler/BRRTRouter/issues/379) | [story-10.5-…](story-10.5-proxy-path-query-passthrough.md) |
| 10.6 | Request-target length → 414 | [#380](https://github.com/microscaler/BRRTRouter/issues/380) | [story-10.6-…](story-10.6-request-target-length-414.md) |
| 10.7 | Error taxonomy | [#381](https://github.com/microscaler/BRRTRouter/issues/381) | [story-10.7-…](story-10.7-error-taxonomy.md) |
| 10.8 | Unify http URI stack | [#382](https://github.com/microscaler/BRRTRouter/issues/382) | [story-10.8-…](story-10.8-unify-http-uri-stack.md) |
| 10.9 | OpenAPI style/explode fidelity | [#383](https://github.com/microscaler/BRRTRouter/issues/383) | [story-10.9-…](story-10.9-openapi-style-explode-fidelity.md) |
| 10.10 | Property/fuzz compliance suite | [#384](https://github.com/microscaler/BRRTRouter/issues/384) | [story-10.10-…](story-10.10-property-fuzz-compliance-suite.md) |
| 10.11 | may_minihttp request-line boundary | [#385](https://github.com/microscaler/BRRTRouter/issues/385) | [story-10.11-…](story-10.11-may-minihttp-request-line-boundary.md) |

**Build:** Wave 0 **done** (10.1). Next → Wave 1 on [BUILD_BOARD.md](../BUILD_BOARD.md).  
**Matrix:** [compliance-matrix.md](../compliance-matrix.md) · **Goldens:** `tests/uri_golden/`

## Primary code surfaces

- `src/server/request.rs` — `parse_query_params`, `parse_request`, `decode_param_value`
- `src/http/proxy.rs` — `resolve_path_template`, `proxy_untyped`
- `src/router/` — path template matching / path params
- may_minihttp (sibling) — request-line → path string handed to BRRTRouter

## References

- Audit: `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md`
- Postmortem: `docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`
- RFC 3986, RFC 9110, WHATWG URL (`application/x-www-form-urlencoded`)
