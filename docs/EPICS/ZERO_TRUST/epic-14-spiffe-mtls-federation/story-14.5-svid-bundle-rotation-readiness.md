# Story 14.5 — SVID/bundle rotation & fail-closed ready

**GitHub issue:** [#418](https://github.com/microscaler/BRRTRouter/issues/418)  
**Epic:** [Epic 14](README.md)  
**Wave:** 3  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Hot-reload trust bundles and optionally client/server SVID material; `/ready`
reflects validity. Expired-only material → not ready / fail-closed.

## Delivery
- Watch or periodic reload of bundle paths.
- Ready callback integration.
- Document file layout compatible with SPIRE delivery.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Bundle file update picked up without restart. |
| FR-2 | All trust material expired → ready=false (or configured). |
| FR-3 | Valid material → ready=true. |
| FR-4 | In-flight requests use atomic snapshot (no torn reads). |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | ArcSwap or equivalent for bundle snapshot. |
| NFR-2 | No panic on corrupt reload (keep last good). |
| NFR-3 | Reload errors metrics/logs without secrets. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Reload new bundle | validates new trust |
| P2 | Ready true with valid | true |
| P3 | Atomic swap | no torn |
| P4 | Last-good on bad reload | still serves old |
| P5 | Metric on reload ok | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Corrupt bundle file | keep last-good; no panic |
| N2 | Empty path | Err/ready false |
| N3 | Ready true with expired-only | forbidden |
| N4 | Panic on reload | forbidden |
| N5 | Log PEM private key | forbidden |

### Acceptance criteria (tests)
- [ ] P1/P2 and N1/N3 mandatory.

## Acceptance criteria
- [ ] Rotation + ready documented; FR/NFR complete.

