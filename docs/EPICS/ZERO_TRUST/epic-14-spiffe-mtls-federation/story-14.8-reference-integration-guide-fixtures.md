# Story 14.8 — Reference integration guide & e2e fixtures

**GitHub issue:** [#421](https://github.com/microscaler/BRRTRouter/issues/421)  
**Epic:** [Epic 14](README.md)  
**Wave:** 6  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Operator guide for SPIRE → BRRTRouter mTLS/JWT, plus deterministic fixtures/e2e
tests that do not require a live cluster (pre-generated material).

## Delivery
- `docs/SPIFFE_MTLS_GUIDE.md` (or similar).
- Fixture certs/bundles in `tests/fixtures/spiffe/`.
- Optional kind/SPIRE lab noted as manual.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Guide covers JWT-only, mTLS, federation file layout. |
| FR-2 | Fixture e2e covers X.509 accept + deny. |
| FR-3 | Sesame/platform pointer for service identity. |
| FR-4 | Troubleshooting: ready=false, wrong SAN. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Fixtures contain no production secrets. |
| NFR-2 | Tests hermetic in CI. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Guide exists | yes |
| P2 | Fixture accept | 2xx |
| P3 | Fixture deny | 401/403 |
| P4 | Ready false scenario | asserted |
| P5 | Links from README | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Prod secrets in fixtures | forbidden |
| N2 | Guide claims issuer mode | forbidden |
| N3 | Broken commands | forbidden |
| N4 | Non-hermetic CI requirement | forbidden |
| N5 | Panic in fixture load | forbidden |

### Acceptance criteria (tests)
- [ ] P2/P3 mandatory.

## Acceptance criteria
- [ ] Guide + hermetic fixtures; FR/NFR complete.

