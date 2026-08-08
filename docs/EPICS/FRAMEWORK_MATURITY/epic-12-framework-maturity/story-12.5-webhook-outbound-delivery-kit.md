# Story 12.5 — Webhook outbound delivery kit

**GitHub issue:** [#396](https://github.com/microscaler/BRRTRouter/issues/396)  
**Epic:** [Epic 12](README.md)  
**Wave:** 2  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Provide a **small platform kit** for sesame-idam-style webhook delivery: HTTP POST
of a JSON payload with optional HMAC signature and bounded retries. This is **not**
OpenAPI Callback Object auto-fire runtime (parked).

Subscription CRUD remains normal OpenAPI paths (already works).

## Delivery

- Module e.g. `src/http/webhook_delivery.rs` (or `src/webhooks/`) using existing
  `may_minihttp` / `fetch_*` client stack.
- Options: URL, headers, body bytes, HMAC secret + header name, max attempts, backoff.
- Document sesame integration pattern (`test_webhook_delivery` controller).
- Explicit non-goals: DLQ UI, OAS `callbacks` expression engine, inbound webhook verify middleware (follow-up).

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | POST JSON to mock server | 2xx; body bytes match |
| P2 | HMAC-SHA256 header set when secret provided | header present + valid |
| P3 | Retry once on 503 then success | eventually OK; attempt count |
| P4 | Custom headers forwarded | seen downstream |
| P5 | Timeout config respected | documented |
| P6 | Idempotency-Key optional header | forwarded when set |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | DNS/connect failure | Err; no panic |
| N2 | Exhausted retries on 500 | Err after N |
| N3 | Invalid URL | Err |
| N4 | Empty secret with HMAC required mode | Err or skip HMAC per API |
| N5 | Oversized body vs client limit | Err |
| N6 | Panic on delivery | forbidden |
| N7 | Silent success on 4xx | forbidden (surface status) |
| N8 | Credential leak in Display/logs | forbidden |

### Acceptance criteria (tests)

- [x] P1/P2 and N1/N2 mandatory.

## Acceptance criteria

- [x] Library API callable from generated controllers (`brrtrouter::http::deliver_webhook`).
- [x] Docs + sesame usage note (`docs/webhook_delivery.md`; Photon `docs/webhooks.md`).
- [x] Unit tests section complete (`tests/webhook_delivery_tests.rs` + module unit tests).

## References

- sesame-idam org-mgmt webhook paths
- `src/http/fetch.rs`, `src/http/proxy.rs` (client patterns)
