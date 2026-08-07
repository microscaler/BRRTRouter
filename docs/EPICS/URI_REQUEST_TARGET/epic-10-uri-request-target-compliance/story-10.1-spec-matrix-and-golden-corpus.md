# Story 10.1 — Spec matrix & golden corpus

**GitHub issue:** _(create)_  
**Epic:** [Epic 10 — URI request-target compliance](README.md)  
**Blocked by:** —  
**Blocks:** 10.2–10.10 (defines Done)

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

## Acceptance criteria

- [ ] Matrix lists every Epic 10 gap from the audit with a unique Requirement ID.
- [ ] Golden corpus checked in; CI runs the harness on `cargo test`.
- [ ] Document states Done = all matrix rows Pass (no “manual only” for core parse/rebuild).
- [ ] Explicit non-goals: RFC 10008 QUERY (Epic 11), HTML form submission quirks beyond form-urlencoded decode.

## References

- `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` §3 scorecard
- RFC 3986 §2, §3.3, §3.4; RFC 9110 §7; WHATWG URL form-urlencoded
