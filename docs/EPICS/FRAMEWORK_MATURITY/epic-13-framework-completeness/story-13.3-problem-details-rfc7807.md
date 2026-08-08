# Story 13.3 — Problem Details (RFC 7807)

**GitHub issue:** [#403](https://github.com/microscaler/BRRTRouter/issues/403)  
**Epic:** [Epic 13](README.md)  
**Wave:** 1  
**Effort:** M  
**Blocked by:** prefers after 13.1 (docs stop claiming until done)  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Emit framework client/server errors as **RFC 7807 Problem Details**
(`application/problem+json`) with stable `type` URIs, while keeping a migration
path for existing `{error, reason, message, fields}` consumers.

## Delivery

- Shared builder e.g. `brrtrouter::http::problem::Problem` / `write_problem`.
- Map existing reasons (`parameter_validation_failed`, `request_body_too_large`,
  multipart errors, 401/403 auth) to `type` + `title` + `status` + `detail`.
- `Content-Type: application/problem+json`.
- Extension members: preserve `reason` and `fields` where useful (RFC allows extensions).
- Config/flag: default problem+json for framework errors; document escape hatch if needed.
- Update marketing claims only after ship.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | Validation **400** responses use `application/problem+json` with `status: 400`. |
| FR-2 | Body too large **413** uses problem+json with stable `type` / `reason`. |
| FR-3 | Auth failures (**401**/**403**) use problem+json when framework-generated. |
| FR-4 | Problem includes at least `type`, `title`, `status`, `detail` (RFC required/recommended set). |
| FR-5 | Field-level errors still expose machine-readable list (`fields` extension). |
| FR-6 | Documented catalog of `type` URIs (relative or `https://microscaler.dev/problems/...`). |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | Builder never panics on empty detail/reason. |
| NFR-2 | No credential leakage in `detail`. |
| NFR-3 | Serialization cost acceptable on error path (not hot success path). |
| NFR-4 | Backward-compatible extension keys (`reason`) for one major cycle unless versioned break noted. |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Param validation failure | 400; CT problem+json; `type`/`status` |
| P2 | Body over limit | 413; problem+json |
| P3 | Multipart missing boundary | 400; problem+json |
| P4 | `fields` present for param errors | array shape |
| P5 | `reason` extension preserved | string match |
| P6 | Catalog doc lists types used in P1–P3 | present |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Claim success body is problem+json | forbidden |
| N2 | Missing `status` member | forbidden |
| N3 | `Content-Type: application/json` only for framework errors | forbidden (must be problem+json) |
| N4 | Panic in builder | forbidden |
| N5 | Token/secret in `detail` | forbidden |
| N6 | Unstable `type` string churn without doc | forbidden |

### Acceptance criteria (tests)

- [x] P1/P2 and N2/N3 mandatory.

## Acceptance criteria

- [x] Framework error paths migrated (or flagged) to problem+json.
- [x] Operator doc + type catalog published.
- [x] FR/NFR + unit tests complete.

## References

- RFC 7807 / RFC 9457
- `src/server/response.rs` (`write_json_error`), `param_validation.rs`, `body_limit.rs`
