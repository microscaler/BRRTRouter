# Framework Maturity — Epics and stories summary

| ID | Title | Effort | Wave | Notes |
|----|--------|--------|------|-------|
| Epic 12 | Framework maturity | — | — | Parent |
| 12.1 | Doc / status reconciliation | S | 0 | README/ROADMAP truth |
| 12.2 | Hard inbound body limits → 413 | S–M | 0 | DoS / memory |
| 12.3 | OpenAPI `$ref` requestBodies / responses / pathItems | M | 1 | Silent schema drop |
| 12.4 | Pre-handler query/header validation | M | 1 | OpenAPI-first E2E |
| 12.5 | Webhook outbound delivery kit | M | 2 | Sesame; not OAS auto-fire |
| 12.6 | Multipart form-data truth | L | 3 | Or hard-fail 415/501 |
| 12.7 | Multi-status typed / codegen | L | 3 | Finish HttpJson story |
| 12.8 | Perf science (Phase 6 benches + validator flamegraph) | M | 4 | Before more micro-opts |

**Parked:** WebSocket, radix rewrite, stack-size plumbing, full OAS callback runtime, OAS 3.2 fleet cutover.
