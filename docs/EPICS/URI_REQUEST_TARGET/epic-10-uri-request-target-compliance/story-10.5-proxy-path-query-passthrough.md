# Story 10.5 — Proxy path/query passthrough

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.2, 10.4, 10.11  
**Blocks:** 10.10

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

## Acceptance criteria

- [ ] Passthrough path documented (when it applies / when rebuild is required).
- [ ] Test: inbound `?q=a%2Bb+c` survives unchanged under passthrough.
- [ ] Template substitution still uses encoders from 10.4.
- [ ] No double-encoding when mixing path substitute + query passthrough.
- [ ] Matrix row “preserve original query octets” marked Pass.

## References

- `src/http/proxy.rs`
- Audit §4.1
