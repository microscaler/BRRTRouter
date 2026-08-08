# Story 16.4 — Semver, changelog, beta checklist

**GitHub issue:** [#433](https://github.com/microscaler/BRRTRouter/issues/433)  
**Epic:** [Epic 16](README.md)  
**Wave:** 3  
**Effort:** S–M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Establish CHANGELOG discipline and a beta/0.1 release checklist.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | CHANGELOG format (Keep a Changelog or equivalent). |
| FR-2 | Beta checklist: tests, docs, publish dry-run, security notes. |
| FR-3 | Version bump procedure documented. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Checklist is executable (commands). |
| NFR-2 | Linked from PUBLISHING.md. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | CHANGELOG exists | yes |
| P2 | Checklist exists | yes |
| P3 | Links PUBLISHING | yes |
| P4 | Lists Epic 13/14/15 deps as optional | yes |
| P5 | Security section | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Empty changelog forever | forbidden |
| N2 | Checklist without tests | forbidden |
| N3 | Broken commands | forbidden |
| N4 | Skip dry-run | forbidden |
| N5 | Claim GA without checklist | forbidden |

### Acceptance criteria (tests)
- [ ] P1/P2 and N2 mandatory.

## Acceptance criteria
- [ ] Process docs complete; FR/NFR complete.

