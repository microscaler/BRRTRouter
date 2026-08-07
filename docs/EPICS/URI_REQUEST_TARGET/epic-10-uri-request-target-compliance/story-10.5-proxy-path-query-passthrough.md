# Story 10.5 — Proxy path/query passthrough

**GitHub issue:** [#379](https://github.com/microscaler/BRRTRouter/issues/379)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.2, 10.4, 10.11  
**Blocks:** 10.10  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Decode→map→encode is necessary when substituting path templates. When the
downstream path needs **no** substitution (or only safe path params), prefer
forwarding the **original** path-and-query octets from the inbound request-target.
That eliminates an entire class of re-serialization bugs for complex queries.

## Delivery

- Retain the raw path-and-query (or raw query) on `ParsedRequest` / `HandlerRequest`
  if not already available from may_minihttp (coordinate with 10.11).
- In `proxy_untyped` / `resolve_path_template`:
  - If downstream template equals inbound path prefix policy and has no `{param}`,
    append inbound query string bytes as received (after validation).
  - If only path params need substitution, rebuild path segments but passthrough
    query when `query_params` were not mutated by middleware.
- Feature flag or always-on with tests proving byte-identical query when
  passthrough applies (including `+` preserved).

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Passthrough: `?q=a%2Bb+c` | query octets unchanged |
| P2 | Passthrough: `%20` preserved | no forced `+` rewrite |
| P3 | Rebuild when template has `{param}` | path encoded via 10.4; Uri-OK |
| P4 | Path substitute + query passthrough | no double-encode of query |
| P5 | Identity mapping multi-param | all params present |
| P6 | Empty query | no spurious `?` (or documented) |
| P7 | Rebuild space → `%20` when passthrough N/A | Uri-OK |
| P8 | Middleware-unmutated query | passthrough selected |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Passthrough raw space in query | reject before send |
| N2 | Passthrough CTL / `#` | reject |
| N3 | Middleware mutated query + passthrough | must rebuild; no stale octets |
| N4 | Missing path param on template path | composition error; no panic |
| N5 | Double-encode mix (rebuild query twice) | forbidden; test detects |
| N6 | Malformed raw query retained | reject |
| N7 | Unknown mode / misconfig | fail closed |
| N8 | Smuggling-ish `?` in path for passthrough | reject or normalize |

### Acceptance criteria (tests)

- [x] P1 mandatory (`+` / `%2B` preservation).
- [x] N3 mandatory (mutation disables passthrough).

## Acceptance criteria

- [x] Passthrough path documented (when it applies / when rebuild is required).
- [x] Test: inbound `?q=a%2Bb+c` survives unchanged under passthrough.
- [x] Template substitution still uses encoders from 10.4.
- [x] No double-encoding when mixing path substitute + query passthrough.
- [x] Matrix row “preserve original query octets” marked Pass.
- [x] Unit tests section complete (positive + negative).

## References

- `src/http/proxy.rs`
- Audit §4.1
