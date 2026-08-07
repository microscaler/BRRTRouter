# Story 10.7 — Error taxonomy (composition vs upstream)

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.6  
**Blocks:** —

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

## Acceptance criteria

- [ ] Uri-build / overlong rebuild never returns 502.
- [ ] DNS failure still 502 (or documented equivalent).
- [ ] Timeout → 504 (if distinguishable) or documented 502 with reason.
- [ ] Matrix row for error taxonomy marked Pass.
- [ ] Loadlinker-style invalid rebuild (if forced) returns 400 in test.

## References

- `src/http/proxy.rs` `ProxyError`, `proxy_untyped`
- Postmortem: `docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`
