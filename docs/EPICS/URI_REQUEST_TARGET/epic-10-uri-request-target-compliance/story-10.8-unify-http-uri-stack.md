# Story 10.8 — Unify http URI stack

**GitHub issue:** [#382](https://github.com/microscaler/BRRTRouter/issues/382)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.4, 10.5 (logic stable first)  
**Blocks:** —  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

BRRTRouter uses `http` **1.0** crate-wide but the proxy client talks
`http_legacy` (**0.2**) for `Uri`/`Method` because of may_minihttp. Dual parsers
risk “passes one, fails the other.” Unify or explicitly bridge with tests that
both stacks accept every golden rebuilt target.

## Delivery

- Inventory all `http_legacy::Uri` / `Method` uses.
- Prefer one of:
  - **A:** may_minihttp gains http 1.x types (upstream PR), or
  - **B:** single internal `RequestTarget` type; convert at the may_minihttp edge only.
- Add a test that every golden outbound target parses in both stacks (while dual
  exists).
- Update Cargo docs / ARCHITECTURE note.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Same logical params → same request-target both stacks | byte-equal |
| P2 | Space → `%20` both stacks | Uri-OK both |
| P3 | Unicode both stacks | Uri-OK both |
| P4 | Path template resolve both stacks | equal |
| P5 | Empty query policy both stacks | equal |
| P6 | Multi-param order both stacks | equal |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Raw space both stacks | same reject-or-encode-before-parse |
| N2 | Truncated `%` decode both | same policy outcome |
| N3 | Oversize both | same error family |
| N4 | Missing param both | same taxonomy |
| N5 | Golden divergence 0.2 ≠ 1.0 | CI fails (hard gate) |
| N6 | Panic on either stack | forbidden |
| N7 | Only one stack tested in CI | forbidden while dual exists |
| N8 | Silent type confusion at bridge | conversion errors typed |

### Acceptance criteria (tests)

- [ ] Shared golden runner against every URI parser still in use.
- [ ] N5 is a hard CI gate for epic Done.

## Acceptance criteria

- [ ] Decision A/B recorded in this story or a short ADR.
- [ ] No silent drift: goldens validated against every URI parser still in use.
- [ ] Matrix row for URI stack unification marked Pass or “Accepted dual-stack with bridge tests.”
- [ ] No behaviour regression in proxy integration tests.
- [ ] Unit tests section complete (positive + negative).

## References

- `Cargo.toml` `http` / `http_legacy`
- `src/http/proxy.rs`
- may_minihttp client API
