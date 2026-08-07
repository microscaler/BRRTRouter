# Story 10.1 — Spec matrix & golden corpus

**GitHub issue:** [#375](https://github.com/microscaler/BRRTRouter/issues/375)  
**Epic:** [Epic 10 — URI request-target compliance](README.md)  
**Blocked by:** —  
**Blocks:** 10.2–10.10 (defines Done)  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Freeze what “100% spec compliant” means as an executable matrix: normative
citations, pass/fail criteria, and a golden vector file checked into the repo.
Later stories only close rows; they do not redefine Done.

## Delivery

- Add `docs/EPICS/URI_REQUEST_TARGET/compliance-matrix.md` (or extend the audit
  scorecard) with columns: Requirement | Spec § | Component | Status | Test ID.
- Add `tests/uri_golden/` (or `src/server/testdata/uri_golden.json`) vectors for:
  - ASCII safe query/path
  - Spaces, tabs, newlines (illegal on wire when raw)
  - Accents / CJK / emoji
  - Reserved in values: `& = ? # / + %`
  - Duplicate keys, empty values, empty keys (if allowed)
  - Overlong percent sequences / truncated `%`
  - `+` vs `%20` space forms
- Wire a single test harness that loads goldens and asserts inbound parse and
  (where applicable) outbound rebuild + `Uri` parse.
- Mark rows already covered by `http::proxy::tests::resolve_path_template_*`
  as Pass with test IDs.

## Unit tests (required)

Harness must run under `cargo test` and fail the build if any golden fails.

### Positive (expect success / correct semantics)

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | ASCII `k=v` | parse → `k`/`v`; rebuild Uri-OK |
| P2 | `%20` space in value | decodes to space; rebuild uses `%20` |
| P3 | `+` space (form) | decodes to space |
| P4 | Accented value (`Côte`) | UTF-8 round-trip |
| P5 | CJK / emoji value | UTF-8 round-trip |
| P6 | Duplicate keys `a=1&a=2` | two entries, order preserved |
| P7 | Empty value `k=` | key present, empty string |
| P8 | Unreserved `-._~` | unchanged through encode |
| P9 | Path segment with encoded space | decode/encode per matrix |
| P10 | Multi-param safe list | all pairs present |

### Negative (expect fail-closed or safe encoding — never panic / never corrupt)

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Truncated `%` (`%`, `%2`) | documented reject or replacement; no panic |
| N2 | Illegal hex `%GG` | documented behaviour; no panic |
| N3 | Raw space in rebuild input | encode → Uri-OK (or composition error if forced raw) |
| N4 | Raw `&` / `=` in value if left unencoded | corruption demonstrated; encoded path safe |
| N5 | Raw `#` in value if left unencoded | fragment truncation demonstrated; encoded safe |
| N6 | Control chars (tab/newline) unencoded | Uri reject or encode path |
| N7 | Oversize target (fixture) | reserved for 10.6; golden flags “length” |
| N8 | Empty input / missing `?` | empty ParamVec; no panic |

### Acceptance criteria (tests)

- [x] Every P* and N* row implemented as a named unit/golden test.
- [x] Harness reports Requirement ID ↔ Test ID for matrix.
- [x] CI runs harness on every PR touching URI/proxy/request code (`cargo test --test uri_golden_harness`).

## Acceptance criteria

- [x] Matrix lists every Epic 10 gap from the audit with a unique Requirement ID.
- [x] Golden corpus checked in; CI runs the harness on `cargo test`.
- [x] Document states Done = all matrix rows Pass (no “manual only” for core parse/rebuild).
- [x] Explicit non-goals: RFC 10008 QUERY (Epic 11), HTML form submission quirks beyond form-urlencoded decode.
- [x] Unit tests section complete (positive + negative).

## Shipped (2026-08-07)

- [`compliance-matrix.md`](../compliance-matrix.md)
- [`tests/uri_golden/corpus.json`](../../../../tests/uri_golden/corpus.json)
- [`tests/uri_golden_harness.rs`](../../../../tests/uri_golden_harness.rs) — 21 tests (P1–P10, N1–N8)

## References

- `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` §3 scorecard
- RFC 3986 §2, §3.3, §3.4; RFC 9110 §7; WHATWG URL form-urlencoded
