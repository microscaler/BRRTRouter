# Story 16.2 — crates.io packaging polish

**GitHub issue:** [#431](https://github.com/microscaler/BRRTRouter/issues/431)  
**Epic:** [Epic 16](README.md)  
**Wave:** 1  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Finish packaging metadata, feature flags, LICENSE, README crates.io section,
and dry-run publish.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Package metadata complete (description, license, repository). |
| FR-2 | `cargo publish -p brrtrouter --dry-run` succeeds. |
| FR-3 | Features (`jemalloc`, `testing`, etc.) documented. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No large accidental blobs in package. |
| NFR-2 | CI optional dry-run job documented. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | dry-run ok | exit 0 |
| P2 | license present | yes |
| P3 | readme for crates.io | yes |
| P4 | exclude patterns | sensible |
| P5 | PUBLISHING.md updated | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Include target/ or kubeconfig | forbidden |
| N2 | Secret env files packaged | forbidden |
| N3 | dry-run fail ignored | forbidden |
| N4 | Missing license | forbidden |
| N5 | Panic in build.rs publish | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N2 mandatory.

## Acceptance criteria
- [ ] Packaging ready; FR/NFR complete.

