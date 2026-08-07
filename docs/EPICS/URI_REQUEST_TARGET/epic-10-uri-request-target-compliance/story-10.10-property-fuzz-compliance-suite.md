# Story 10.10 — Property/fuzz compliance suite

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.2, 10.3, 10.4, 10.5, 10.9  
**Blocks:** Epic 10 Done

## Overview

Table tests catch known geography/reserved cases; property/fuzz tests prove the
absence of “South Africa”-class regressions for arbitrary Unicode and reserved
characters. This story is the automated proof that the matrix is Pass.

## Delivery

- Add `proptest` (or equivalent) property tests:
  1. For random strings from an alphabet including spaces, `&?=#+/%`, and
     Unicode (accents, CJK, emoji):  
     encode query → build path `/?k=<enc>` → `parse_query_params` → value equals
     original (or documented normalization).
  2. `resolve_path_template` output always `Uri`-parseable.
  3. Passthrough mode (10.5): when applicable, query bytes unchanged.
- Optional: `cargo fuzz` target for `parse_query_params` (no panic).
- CI: property tests in default `cargo test`; fuzz optional nightly/job.

## Acceptance criteria

- [ ] Property tests in CI with fixed RNG seed for reproducibility + unseeded local runs.
- [ ] No known counterexample for legal inputs after 10.2–10.5.
- [ ] Fuzz or property “no panic” on inbound parse.
- [ ] Matrix marked Pass; audit scorecard updated.

## References

- `src/http/proxy.rs` tests
- `src/server/request.rs` tests
- Story 10.1 golden corpus
