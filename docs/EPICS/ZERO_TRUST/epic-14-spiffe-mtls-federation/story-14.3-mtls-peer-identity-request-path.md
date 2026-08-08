# Story 14.3 — mTLS peer identity on request path

**GitHub issue:** [#416](https://github.com/microscaler/BRRTRouter/issues/416)  
**Epic:** [Epic 14](README.md)  
**Wave:** 1  
**Effort:** L  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Expose verified peer certificate (or pre-validated SPIFFE ID) from the TLS
connection into `AppService` request context. May require may_minihttp TLS hooks —
document fork needs.

## Delivery
- Request context field for peer SPIFFE ID / cert fingerprint.
- Config: require mTLS for route/security scheme.
- Integration with existing HTTP server accept path.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | When mTLS enabled and peer presents valid cert, ID available to auth. |
| FR-2 | Missing peer cert when required → reject before handler. |
| FR-3 | Plain HTTP / no TLS path documented (dev-only escape). |
| FR-4 | Does not break JWT Bearer routes on same server when both configured. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No panic if TLS layer omits peer cert. |
| NFR-2 | Document may_minihttp capability gaps as blockers with issues. |
| NFR-3 | Allocation of peer ID uses Arc/cheap clone on hot path. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Mock peer cert → context populated | present |
| P2 | Optional mTLS off → HTTP works | ok |
| P3 | Required mTLS + valid peer | proceeds to auth |
| P4 | Coexist with JWT route | both work |
| P5 | Fingerprint/ID stable across requests | equal |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Required mTLS; no peer | 401/403; no handler |
| N2 | Invalid peer cert | reject; no panic |
| N3 | Panic on None peer | forbidden |
| N4 | Trust peer without validation | forbidden |
| N5 | Log private key | forbidden |

### Acceptance criteria (tests)
- [ ] P1/P3 and N1/N3 mandatory.

## Acceptance criteria
- [ ] Peer identity on path; may_minihttp gaps tracked; FR/NFR complete.

