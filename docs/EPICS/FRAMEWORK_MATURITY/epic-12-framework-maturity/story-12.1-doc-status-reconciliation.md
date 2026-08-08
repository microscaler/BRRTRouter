# Story 12.1 — Doc / status reconciliation

**GitHub issue:** [#392](https://github.com/microscaler/BRRTRouter/issues/392)  
**Epic:** [Epic 12](README.md)  
**Wave:** 0  
**Effort:** S  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Align README feature table, ROADMAP, and CONTRIBUTING with reality so contributors
invest in the right gaps (not “regex routing”, “typed panics don’t catch”, etc.).

## Delivery

- Update README: radix (not regex), typed panic recovery ✅, stack-size story accurate, SSE buffered caveat, WS parked.
- Archive or rewrite stale `docs/ROADMAP.md` sections that list shipped work as planned.
- Point CONTRIBUTING “good first issues” at Epic 12 board.
- Cross-link [`OPENAPI_VERSION_SUPPORT.md`](../../../OPENAPI_VERSION_SUPPORT.md) and this theme.

## Unit tests (required)

Docs-only story: treat doc fixtures / link checks as the test surface.

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | README mentions radix / PathCursor | present |
| P2 | README does not claim regex matchers as runtime | absent or corrected |
| P3 | Stack-size section points at `docs/stack_size.md` + vendor ext | present |
| P4 | Epic 12 BUILD_BOARD linked from EPICS_CATALOG | present |
| P5 | WS marked parked / not “in progress” falsely | present |
| P6 | Typed panic recovery described accurately | present |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | ROADMAP still lists CORS/metrics as unstarted if shipped | forbidden |
| N2 | CONTRIBUTING steers to WS as primary MVP gap | forbidden |
| N3 | Conflicting “stack size 🚧” vs shipped override | forbidden |
| N4 | Broken relative links in updated docs | forbidden |
| N5 | Epic 10/11 marked incomplete incorrectly | forbidden |
| N6 | Silent deletion of useful roadmap items without archive note | forbidden |

### Acceptance criteria (tests)

- [x] Doc lint / `rg` fixture test (or checklist PR) covers P1–P3 and N1–N3.

## Acceptance criteria

- [x] README feature table matches engineering truth.
- [x] ROADMAP either regenerated or clearly dated/archived for stale rows.
- [x] Unit tests section complete.
