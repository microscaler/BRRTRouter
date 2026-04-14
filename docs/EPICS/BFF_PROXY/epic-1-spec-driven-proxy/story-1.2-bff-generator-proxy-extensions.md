# Story 1.2 — BFF generator proxy extensions

**GitHub issue:** [#260](https://github.com/microscaler/BRRTRouter/issues/260)  
**Epic:** [Epic 1 — Spec-driven proxy](README.md)

## Overview

The BFF OpenAPI spec is generated from downstream service specs. At merge time, the generator must emit per-operation `x-brrtrouter-downstream-path` (exact path on the downstream service) and `x-service` (service key for host lookup) so BRRTRouter can drive the proxy from the spec.

## Delivery

- Extend the BFF generator (e.g. RERP `generate_system.py` or standalone bff-generator) so that for each merged operation it sets:
  - `x-brrtrouter-downstream-path`: exact path on the downstream (e.g. `base_path + path` normalised, e.g. `/api/invoice/invoices/{id}`).
  - `x-service`: service key (e.g. `invoice`) used by runtime config to resolve host/port.
- Generator already has access to per-service `base_path` and path from the sub-service spec; this story adds writing these two extensions to the merged BFF spec.

## Acceptance criteria

- [ ] Generated BFF spec contains `x-brrtrouter-downstream-path` and `x-service` on each operation that corresponds to a downstream service.
- [ ] `x-brrtrouter-downstream-path` is the exact path on the downstream (path only, no host).
- [ ] `x-service` matches the service key used in BFF suite config (e.g. `bff-suite-config.yaml`) for base URL resolution.
- [ ] Re-generation from existing RERP/openapi sources produces a valid spec consumable by BRRTRouter (Story 1.1).

## Example config (OpenAPI)

Merged BFF operation example:

```yaml
paths:
  /api/bff/invoices/{id}:
    get:
      operationId: getInvoice
      x-brrtrouter-downstream-path: "/api/invoice/invoices/{id}"
      x-service: invoice
      parameters: []
      responses:
        '200':
          description: OK
```

## Diagram

```mermaid
sequenceDiagram
  participant Sub as Sub-service specs
  participant Gen as BFF generator
  participant BFFSpec as BFF OpenAPI spec

  Gen->>Sub: Read paths + base_path per service
  Gen->>Gen: Merge paths; compute downstream_path = base_path + path
  Gen->>BFFSpec: Write operation + x-brrtrouter-downstream-path, x-service
```

## References

- RERP: `openapi/accounting/bff-suite-config.yaml`, `tooling/src/rerp_tooling/bff/generate_system.py` (or equivalent)
- `docs/BFF_PROXY_ANALYSIS.md` §3.2, §5.2, §5.6
