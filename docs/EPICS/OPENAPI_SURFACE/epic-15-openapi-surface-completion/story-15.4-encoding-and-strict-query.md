# Story 15.4 — encoding object + strict query option

**GitHub issue:** [#425](https://github.com/microscaler/BRRTRouter/issues/425)  
**Epic:** [Epic 15](README.md)  
**Wave:** 2  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Document/enforce OpenAPI multipart `encoding` subset; optional reject unknown query keys.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Supported `encoding` features listed; unsupported → warn or hard-fail (configurable). |
| FR-2 | Optional strict query: unknown keys → 400. |
| FR-3 | Default remains permissive for query (compat). |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Strict mode off by default. |
| NFR-2 | No panic on encoding map oddities. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Known encoding subset honored | ok |
| P2 | Strict query known keys | ok |
| P3 | Permissive default unknown query | ok |
| P4 | Docs list supported encoding | yes |
| P5 | Multipart regression | ok |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Strict + unknown query | 400 |
| N2 | Unsupported encoding + hard-fail mode | 4xx/err |
| N3 | Panic | forbidden |
| N4 | Strict on by default | forbidden |
| N5 | Silent accept when hard-fail on | forbidden |

### Acceptance criteria (tests)
- [ ] P3/P2 and N1 mandatory.

## Acceptance criteria
- [ ] Config + docs; FR/NFR complete.

