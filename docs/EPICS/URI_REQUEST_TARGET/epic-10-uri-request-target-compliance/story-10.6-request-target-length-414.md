# Story 10.6 — Request-target length → 414

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1  
**Blocks:** 10.7

## Overview

RFC 9110 recommends supporting request targets of at least ~8000 octets;
implementations and peers vary. BRRTRouter must enforce a configurable maximum
and return **414 URI Too Long** before proxy dial or heavy processing, instead
of opaque client/proxy failures.

## Delivery

- Config knob (env and/or config file), default ≥ 8192 octets of request-target
  (path + `?` + query).
- Enforce on inbound `parse_request` and on outbound rebuilt target in proxy.
- Metrics/log when rejected (no body leak of full target at info if sensitive).
- Tests: under limit succeeds; over limit → 414.

## Acceptance criteria

- [ ] Configurable max with documented default (≥ 8192).
- [ ] Inbound over-limit → 414 (not 500/502).
- [ ] Outbound rebuilt target over-limit → composition error (see 10.7), not dial.
- [ ] Matrix row for length limits marked Pass.

## References

- RFC 9110 §7 (request target) length guidance
- `src/server/request.rs`, `src/http/proxy.rs`
