# Story 10.4 — Component-specific encoders

**GitHub issue:** [#378](https://github.com/microscaler/BRRTRouter/issues/378)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1, 10.2  
**Blocks:** 10.5, 10.10  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Replace the single `urlencoding::encode` call site with explicit **path-segment**
and **query-component** encoders (even if both start as “encode all but
unreserved”). Document the intentional decode/encode asymmetry:
inbound `+` → space; outbound space → `%20` (never `+` on rebuilt request-targets).

## Delivery

- Introduce `brrtrouter::http::uri_encode` (or similar) with:
  - `encode_path_segment(&str) -> String`
  - `encode_query_component(&str) -> String` (keys and values)
- Use them from `resolve_path_template`.
- Module docs cite RFC 3986 unreserved set and forbid `+` for space on rebuild.
- Keep existing positive/negative proxy tests green; add encoder unit tests.

## Unit tests (required)

Module: encoder helpers + `resolve_path_template`.

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Query value `South Africa` | `%20`; Uri-OK (provinces regression) |
| P2 | Accented value | UTF-8 pct-encoded; Uri-OK |
| P3 | Path segment with space | segment encoder; Uri-OK |
| P4 | `+` in logical value | outbound `%2B` (not space) |
| P5 | Unreserved `-._~` | not over-encoded (or over-encode documented OK) |
| P6 | Empty value | `k=` Uri-OK |
| P7 | Multi-param rebuild | Uri-OK; order stable |
| P8 | Path-only template | Uri-OK |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Raw space left unencoded | `Uri::try_from` fails — proves encoder necessity |
| N2 | Raw `&` in value unencoded | param split — encoder must pct-encode |
| N3 | Raw `=` in value unencoded | pair corruption — must encode |
| N4 | Raw `#` in value unencoded | fragment truncation — must encode |
| N5 | Raw `?` in path segment | corruption — must encode |
| N6 | Tab/newline unencoded | Uri reject; encoded path Uri-OK |
| N7 | Missing required path param | composition error; no panic |
| N8 | Direct `urlencoding::encode` in proxy | lint/review: forbidden after refactor |

### Acceptance criteria (tests)

- [ ] Existing `resolve_path_template_*` tests mapped to P*/N*.
- [ ] N1–N4 mandatory; P1 mandatory regression.

## Acceptance criteria

- [ ] Path and query encoding go through named APIs (no raw `urlencoding::encode` in proxy).
- [ ] Space always `%20` on rebuild; `+` in a value encodes as `%2B`.
- [ ] Docs state form-urlencoded inbound vs URI-component outbound policy.
- [ ] Matrix rows for outbound encoding marked Pass (pending passthrough 10.5).
- [ ] Unit tests section complete (positive + negative).

## References

- `src/http/proxy.rs` `resolve_path_template`
- RFC 3986 §2.3 (unreserved), §2.4
- `docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`
