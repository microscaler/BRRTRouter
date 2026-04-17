# Epic 6 — IDAM contract and reference spec

**GitHub issue:** [#278](https://github.com/microscaler/BRRTRouter/issues/278)

**Catalog:** [Epics 1–9](../EPICS_CATALOG.md) | **Theme:** IDAM (Epics 6–9)

## Overview

Document the IDAM contract expected by the BFF (endpoints, behaviour, JWKS) so BFF/BRRTRouter remain agnostic of which IDAM implementation each system uses. Produce a reference IDAM core OpenAPI derived from GoTrue (path prefix, minimal surface) so the core/extension split is explicit and implementable.

## Scope

- **BRRTRouter/Microscaler docs:** Add "IDAM contract" section: endpoints and behaviour needed for auth, RBAC/claims, JWKS; BFF never calls Supabase directly.
- **Reference spec:** Derive reference `idam-core.openapi.yaml` from GoTrue openapi.yaml (path prefix, e.g. `/api/identity/auth/*`); document path layout for core vs extension (see [IDAM GoTrue API Mapping](../../../IDAM_GOTRUE_API_MAPPING.md)).

## Stories

| Story | Title | Doc | GitHub issue |
|-------|--------|-----|--------------|
| 6.1 | Document IDAM contract | [story-6.1-document-idam-contract.md](story-6.1-document-idam-contract.md) | [#282](https://github.com/microscaler/BRRTRouter/issues/282) |
| 6.2 | Reference IDAM core OpenAPI | [story-6.2-reference-idam-core-openapi.md](story-6.2-reference-idam-core-openapi.md) | [#283](https://github.com/microscaler/BRRTRouter/issues/283) |

## References

- [IDAM Microscaler Analysis](../../../IDAM_MICROSCALER_ANALYSIS.md) §4
- [IDAM GoTrue API Mapping](../../../IDAM_GOTRUE_API_MAPPING.md) §1, §3
- [BFF Proxy Analysis](../../../BFF_PROXY_ANALYSIS.md) §6
