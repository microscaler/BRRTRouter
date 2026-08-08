# Story 10.10 — Property/fuzz compliance suite

**GitHub issue:** [#384](https://github.com/microscaler/BRRTRouter/issues/384)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.2, 10.3, 10.4, 10.5, 10.9  
**Blocks:** Epic 10 Done  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

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

## Unit tests (required)

Property tests that run under `cargo test` count as unit tests.

### Positive (properties / cases)

| ID | Property / case | Assert |
|----|-----------------|--------|
| P1 | Random Unicode + reserved (bounded) encode→parse | logical equality (or documented norm) |
| P2 | `resolve_path_template` always Uri-OK for legal maps | holds |
| P3 | Passthrough (when applicable) | query bytes unchanged |
| P4 | Golden corpus still Pass | no regression |
| P5 | Duplicate keys survive rebuild | multiset equality |
| P6 | Fixed RNG seed CI run | reproducible |

### Negative (properties / cases)

| ID | Property / case | Assert |
|----|-----------------|--------|
| N1 | Arbitrary `&str` path to parse | no panic |
| N2 | Arbitrary query suffix | no panic |
| N3 | Encoder never emits raw space in query values | holds |
| N4 | Encoder never emits raw `&`/`=` inside values | holds or documented structure |
| N5 | Invalid UTF-8 / binary if accepted as bytes | no panic |
| N6 | Fuzz crash → minimized golden added | process + test |
| N7 | Oversize random strings | hit 10.6; no OOM in budget |
| N8 | Shrink failing case stores seed | CI artifact/docs |

### Acceptance criteria (tests)

- [x] `cargo test` runs property suite (feature-gated OK if default-on in CI).
- [x] N1/N2 hard requirements.
- [x] N6 process documented.

## Acceptance criteria

- [x] Property tests in CI with fixed RNG seed for reproducibility + unseeded local runs.
- [x] No known counterexample for legal inputs after 10.2–10.5.
- [x] Fuzz or property “no panic” on inbound parse.
- [x] Matrix marked Pass; audit scorecard updated.
- [x] Unit tests section complete (positive + negative).

## References

- `tests/uri_property_tests.rs` (`RngSeed::Fixed(0x1010_c0ff_ee)`)
- `src/http/proxy.rs` tests
- `src/server/request.rs` tests
- Story 10.1 golden corpus (`tests/uri_golden/`)
