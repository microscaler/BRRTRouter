# Story 13.5 — Browser security posture

**GitHub issue:** [#405](https://github.com/microscaler/BRRTRouter/issues/405)  
**Epic:** [Epic 13](README.md)  
**Wave:** 3  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Decide and ship a clear **browser security posture**: either a small **cookie +
CSRF kit** for cookie-session BFFs, or an explicit **Bearer/JWKS-only** contract
with Set-Cookie helpers only for non-session use. No silent half-session framework.

**Default product path remains Sesame-style Bearer/JWKS** — sessions are not required.

## Delivery (choose one outcome — document which)

### Option A — Kit (preferred if any cookie-session BFF needs it)

- `Set-Cookie` builder: `Secure`, `HttpOnly`, `SameSite`, `Path`, `Max-Age`.
- CSRF: double-submit cookie or synchronizer token middleware for unsafe methods.
- Docs: when to use vs Bearer.

### Option B — Explicit out-of-scope

- Operator doc: “no server sessions; use Sesame/IDAM tokens.”
- Cookie parse remains for API-key/JWT-in-cookie providers only.
- Marketing must not imply session middleware.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | Published posture doc states Option A or B unambiguously. |
| FR-2a | *(A)* Cookie builder emits RFC-compliant `Set-Cookie` string/header. |
| FR-3a | *(A)* Unsafe method without valid CSRF → **403** with stable error/problem. |
| FR-4a | *(A)* Safe methods (GET/HEAD) do not require CSRF. |
| FR-2b | *(B)* Docs forbid claiming session middleware; README feature table accurate. |
| FR-5 | Existing auth-from-cookie providers keep working (regression). |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | CSRF secrets never logged. |
| NFR-2 | Cookie flags default secure in production profile guidance. |
| NFR-3 | No panic on malformed Cookie header. |
| NFR-4 | Kit (if A) has no dependency on Redis/external session store in v1. |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Posture doc exists and picks A or B | present |
| P2a | *(A)* Builder sets HttpOnly+Secure+SameSite | headers |
| P3a | *(A)* Valid CSRF on POST | proceeds |
| P4a | *(A)* GET without CSRF | proceeds |
| P2b | *(B)* Feature table omits sessions | absent claim |
| P5 | Cookie auth provider still authenticates | regression |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1a | *(A)* POST missing CSRF | **403** |
| N2a | *(A)* CSRF token mismatch | **403** |
| N3 | Malformed Cookie header | no panic |
| N4 | Claim “sessions included” if Option B | forbidden |
| N5 | Panic in cookie builder | forbidden |
| N6 | CSRF secret in error body | forbidden |

### Acceptance criteria (tests)

- [x] P1 mandatory; if A → P3a/N1a mandatory; if B → P2b/N4 mandatory.

## Acceptance criteria

- [x] One posture shipped and documented (**Option B**).
- [x] FR/NFR + unit tests complete for the chosen option.

## References

- `src/security/*` cookie_name APIs, `src/server/request.rs` cookie parse
- Sesame-IDAM auth patterns
