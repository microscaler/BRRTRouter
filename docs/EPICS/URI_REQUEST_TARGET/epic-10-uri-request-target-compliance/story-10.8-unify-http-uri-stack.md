# Story 10.8 — Unify http URI stack

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.4, 10.5 (logic stable first)  
**Blocks:** —

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

## Acceptance criteria

- [ ] Decision A/B recorded in this story or a short ADR.
- [ ] No silent drift: goldens validated against every URI parser still in use.
- [ ] Matrix row for URI stack unification marked Pass or “Accepted dual-stack with bridge tests.”
- [ ] No behaviour regression in proxy integration tests.

## References

- `Cargo.toml` `http` / `http_legacy`
- `src/http/proxy.rs`
- may_minihttp client API
