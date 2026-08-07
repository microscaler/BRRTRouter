# Story 10.7 — Error taxonomy (composition vs upstream)

**GitHub issue:** [#381](https://github.com/microscaler/BRRTRouter/issues/381)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.6  
**Blocks:** —  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

`proxy_untyped` maps every `ProxyError` to HTTP **502**, which made Uri-build
failures look like gateway death (`invalid path: invalid uri character`).
Split composition/client errors from upstream/transport errors.

## Delivery

- Classify errors:
  - **Composition** (invalid rebuilt URI, overlong target, illegal encoding on
    rebuild): **400** (or problem+json) — not 502.
  - **DNS / connect / timeout**: **502** / **504** as appropriate.
  - **Upstream HTTP status**: pass through (existing behaviour).
- Keep stable `error` / `title` strings for ops grep (`invalid path:` may remain
  in body for composition).
- Update unit test `proxy_untyped_returns_502_on_dns_failure`; add composition → 400.
- Document in proxy module docs + audit scorecard.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | DNS failure | still **502** (regression) |
| P2 | Timeout (if distinguishable) | **504** or documented 502 + reason |
| P3 | Upstream 4xx/5xx | pass through |
| P4 | Valid rebuild | proxy continues (no composition error) |
| P5 | Overlong rebuild | composition status (400/414 per table) |
| P6 | Stable metric/label per variant | asserted |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Forced invalid rebuild (raw space) | **400** (not 502) — Loadlinker-class |
| N2 | Missing path param | composition status; not 502 |
| N3 | Illegal encoding on rebuild | composition status |
| N4 | Catch-all `InvalidPath` without reason | forbidden — reason code required |
| N5 | Error Display leaks secrets/full URI | redacted / short reason |
| N6 | Wrong status in mapping table | table-driven test fails |
| N7 | Panic in error path | forbidden |
| N8 | Body JSON shape invalid | schema/shape assertion |

### Acceptance criteria (tests)

- [ ] Table-driven mapping for every URI-related `ProxyError` variant.
- [ ] N1 mandatory (provinces-era classification).

## Acceptance criteria

- [ ] Uri-build / overlong rebuild never returns 502.
- [ ] DNS failure still 502 (or documented equivalent).
- [ ] Timeout → 504 (if distinguishable) or documented 502 with reason.
- [ ] Matrix row for error taxonomy marked Pass.
- [ ] Loadlinker-style invalid rebuild (if forced) returns 400 in test.
- [ ] Unit tests section complete (positive + negative).

## References

- `src/http/proxy.rs` `ProxyError`, `proxy_untyped`
- Postmortem: `docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`
