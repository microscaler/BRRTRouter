# Story 10.6 — Request-target length → 414

**GitHub issue:** [#380](https://github.com/microscaler/BRRTRouter/issues/380)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1  
**Blocks:** 10.7  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

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

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Target under limit | success |
| P2 | Target at limit − 1 | success |
| P3 | Target exactly at limit | success (or exclusive bound documented) |
| P4 | Short path + short query | success |
| P5 | Encoded expansion under limit | counts **wire** length |
| P6 | Default ≥ 8192 | config/default assertion |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Inbound over limit | **414**; not 500/502; no panic |
| N2 | Outbound rebuild over limit | composition error before dial |
| N3 | Limit misconfig (0 / negative) | fail closed at load or reject safely |
| N4 | Extremely large fixture | bounded memory in test |
| N5 | Repeated keys over limit | 414 |
| N6 | Path alone over limit | 414 |
| N7 | Encoded longer than decoded | wire length wins |
| N8 | Log/metric on reject | fires; no full sensitive target at info |

### Acceptance criteria (tests)

- [x] N1/N2 mandatory; P6 locks default.

## Acceptance criteria

- [x] Configurable max with documented default (≥ 8192).
- [x] Inbound over-limit → 414 (not 500/502).
- [x] Outbound rebuilt target over-limit → composition error (see 10.7), not dial.
- [x] Matrix row for length limits marked Pass.
- [x] Unit tests section complete (positive + negative).

## References

- RFC 9110 §7 (request target) length guidance
- `src/server/request.rs`, `src/http/proxy.rs`
