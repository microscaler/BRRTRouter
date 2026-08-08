# Story 12.2 — Hard inbound body limits → 413

**GitHub issue:** [#393](https://github.com/microscaler/BRRTRouter/issues/393)  
**Epic:** [Epic 12](README.md)  
**Wave:** 0  
**Effort:** S–M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Enforce hard caps on inbound request bodies **before** full JSON parse / handler
dispatch. Today `estimated_request_body_bytes` / `x-brrtrouter-body-size-bytes`
feed logging — not **413 Payload Too Large**.

## Delivery

- Global max (env / config) + per-route estimate / vendor override.
- Prefer `Content-Length` when present; stream/read cap when absent.
- Stable JSON error `{error, reason, message}` aligned with Epic 10 taxonomy style.
- Document knobs in `docs/` + OPENAPI extension wiki.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Body under global max | 2xx/route proceeds |
| P2 | Body under route estimate | proceeds |
| P3 | Exact Content-Length at limit | accepted or documented boundary |
| P4 | Vendor `x-brrtrouter-body-size-bytes` raises cap | respected |
| P5 | Small JSON POST pet-store class | regression OK |
| P6 | Empty body on GET | unaffected |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Content-Length over global max | **413**; no handler |
| N2 | Content-Length over route cap | **413** |
| N3 | Chunked/absent CL + read past cap | **413**; no panic |
| N4 | Hostile huge CL header | reject/413; no OOM |
| N5 | Body after 413 not partially applied to state | no side effects |
| N6 | Error JSON shape stable | `error`/`reason` present |
| N7 | Panic on limit path | forbidden |
| N8 | Silent truncate | forbidden |

### Acceptance criteria (tests)

- [ ] N1/N2 mandatory; P1 mandatory.

## Acceptance criteria

- [ ] Oversize inbound → 413 before handler.
- [ ] Config/env documented.
- [ ] Unit tests section complete.

## References

- `src/server/service.rs`, `src/server/request.rs`, `src/spec/build.rs` (`estimate_body_size`)
