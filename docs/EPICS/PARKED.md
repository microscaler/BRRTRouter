# Parked work (no active epic)

Explicit non-goals until a future decision:

| Item | Why parked | Notes |
|------|------------|-------|
| **WebSocket upgrade** | Needs may_minihttp upgrade; product choice | Accepted out of near-term scope |
| **OAS callback auto-fire engine** | High complexity; outbound kit ships (12.5) | Epic 15.7 is object fidelity only |
| **Radix trie rewrite** | Perf science (12.8): match already sub-µs | Optimize e2e/dispatch instead |
| **Stack-size product APIs** | Mostly done; plumbing parked | See stack_size.md |

Active follow-on epics from the framework gap audit:

- Epic 13 — Framework completeness (ops/DevEx)
- Epic 14 — SPIFFE X.509 / mTLS / Federation (**critical**)
- Epic 15 — OpenAPI surface completion
- Epic 16 — Release & observability maturity

BFF claim enrichment remains under **Epics 3–5 / 6–9** (not recreated here).
