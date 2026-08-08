## 2026-08-09 — Wave 0 done; JWT = consumer only

**Policy:** Sesame-IDAM / external IdP **issues** JWTs. BRRTRouter **validates & enforces**
only (`docs/JWT_AND_IDENTITY_BOUNDARY.md`). No in-router IdP / revocation product.

| Story | Issue | Commit |
|-------|-------|--------|
| 13.1 Doc truth | #401 closed | `fcf70fa` |
| 14.1 Zero-trust inventory | #414 closed | `fcf70fa` |

**NOW:** 13.2 rate limiting (#402) · 14.2 X.509 SVID (#415)  
**Epics:** 13 #400 · 14 #411 · 15 #412 · 16 #413  
BFF claim enrichment (3–9): product track when Sesame needs it — not gating 13–16.

## Prior — Epic 13–16 planned

Stories + boards under `docs/EPICS/`; catalog `docs/EPICS/EPICS_CATALOG.md`.
