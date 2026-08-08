# Story 15.3 — servers / basePath overrides

**GitHub issue:** [#424](https://github.com/microscaler/BRRTRouter/issues/424)  
**Epic:** [Epic 15](README.md)  
**Wave:** 1  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Apply OpenAPI `servers` URL path prefix / basePath consistently at route build.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Spec with server url `/api/v1` prefixes routes. |
| FR-2 | Multiple servers: documented selection (first / config override). |
| FR-3 | Empty/relative server handled without panic. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Deterministic prefix join (no double slashes). |
| NFR-2 | Hot reload rebuilds with new servers. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Prefix applied | match `/api/v1/...` |
| P2 | Override config | honored |
| P3 | No servers | legacy paths |
| P4 | Hot reload | updated |
| P5 | Trailing slash normalize | ok |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Hostile server URL | Err/skip; no panic |
| N2 | Double prefix | forbidden |
| N3 | Panic on empty | forbidden |
| N4 | Silent ignore servers when present | forbidden |
| N5 | Break absolute path routes wrongly | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N2/N3 mandatory.

## Acceptance criteria
- [ ] Documented selection policy; FR/NFR complete.

