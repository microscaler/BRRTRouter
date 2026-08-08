## 2026-08-09 — Epic 13.3 Problem Details shipped

**Commit:** `218c3b6` · Issue [#403](https://github.com/microscaler/BRRTRouter/issues/403) closed.

- `brrtrouter::http::problem::Problem` + `write_problem`
- Framework errors → `application/problem+json` (`HandlerResponse::error`, `write_json_error`)
- Catalog: `docs/PROBLEM_DETAILS.md`
- Escape hatch: `BRRTR_LEGACY_ERROR_JSON=1`

**NOW:** 13.4 streaming uploads/downloads (#404) · 14.2 X.509 SVID (#415)  
**JWT:** consumer/enforcer only (Sesame/external IdP)
