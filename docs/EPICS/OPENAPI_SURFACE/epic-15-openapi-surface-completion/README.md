# Epic 15 — OpenAPI surface completion

**GitHub issue:** [#412](https://github.com/microscaler/BRRTRouter/issues/412)  
**Theme labels:** `openapi-surface`, `epic`  
**Testing:** [`../TESTING_STANDARD.md`](../TESTING_STANDARD.md)  
**Board:** [`../BUILD_BOARD.md`](../BUILD_BOARD.md)

## Overview

Round out OpenAPI **contract surface** left after Epics 10–13: response headers,
`servers`/basePath, `encoding`, HTTP method matrix, versioning patterns,
callbacks/webhooks **object fidelity** (not auto-fire runtime), and OAS 3.2
readiness. Complements Epic 13 (ops/DevEx) without overlapping rate-limit/files.

**Does not include:** WebSocket; full callback expression auto-fire engine (parked
unless a later epic); fleet-wide forced 3.2 cutover without oas3 support.

## Success criteria (epic-level)

- [ ] OPENAPI compliance gap inventory matches code for remaining surfaces.
- [ ] Declared response headers can be validated or generated.
- [ ] `servers` / basePath overrides affect route build predictably.
- [ ] Method matrix documented (OPTIONS Allow, TRACE policy).
- [ ] Versioning pattern documented and optionally helper-supported.
- [ ] OAS 3.2 dual-support plan written; QUERY path remains valid on 3.1.

## Wave plan

```text
Wave 0 ──► 15.1 gap inventory (remaining surface)
Wave 1 ──► 15.2 response headers ‖ 15.3 servers/basePath
Wave 2 ──► 15.4 encoding + strict query option
Wave 3 ──► 15.5 method matrix ‖ 15.6 versioning pattern
Wave 4 ──► 15.7 callbacks/webhooks object fidelity
Wave 5 ──► 15.8 OAS 3.2 readiness plan
```

## Stories

| Story | Title | Issue | Effort | Blocked by |
|-------|--------|-------|--------|------------|
| 15.1 | Remaining OpenAPI gap inventory | [#422](https://github.com/microscaler/BRRTRouter/issues/422) | S | benefits from 13.1 |
| 15.2 | Response headers fidelity | [#423](https://github.com/microscaler/BRRTRouter/issues/423) | M | 15.1 |
| 15.3 | servers / basePath overrides | [#424](https://github.com/microscaler/BRRTRouter/issues/424) | M | — |
| 15.4 | encoding object + strict query option | [#425](https://github.com/microscaler/BRRTRouter/issues/425) | M | 12.6/13.4 helpful |
| 15.5 | HTTP method matrix (OPTIONS/TRACE) | [#426](https://github.com/microscaler/BRRTRouter/issues/426) | S–M | — |
| 15.6 | API versioning pattern | [#427](https://github.com/microscaler/BRRTRouter/issues/427) | S–M | 15.3 helpful |
| 15.7 | callbacks/webhooks object fidelity | [#428](https://github.com/microscaler/BRRTRouter/issues/428) | M | — |
| 15.8 | OAS 3.2 dual-support readiness | [#429](https://github.com/microscaler/BRRTRouter/issues/429) | M | 15.1 |

## Functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-FR-1 | Operators can see an accurate remaining-gap inventory. |
| E-FR-2 | Response `headers` in OpenAPI influence runtime and/or codegen. |
| E-FR-3 | Spec `servers`/basePath can prefix routes consistently. |
| E-FR-4 | Multipart `encoding` supported subset is documented and enforced or explicitly ignored with warn. |
| E-FR-5 | OPTIONS can advertise `Allow`; TRACE policy is explicit. |
| E-FR-6 | Versioning guidance exists (path or servers). |
| E-FR-7 | `callbacks` / root `webhooks` parsed or clearly rejected — not silently dropped without note. |

## Non-functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-NFR-1 | No silent drop of declared contract elements without doc/warn. |
| E-NFR-2 | No panics on odd OAS shapes. |
| E-NFR-3 | Stay compatible with OpenAPI 3.1 fleet default. |

## Parked inside this theme

- Full OAS callback **auto-fire** runtime (expression engine) — product kit remains Epic 12.5 outbound webhooks.
