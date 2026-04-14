# Epic 7 — IDAM core implementation (GoTrue proxy)

**GitHub issue:** [#279](https://github.com/microscaler/BRRTRouter/issues/279)

**Catalog:** [Epics 1–9](../EPICS_CATALOG.md) | **Theme:** IDAM (Epics 6–9)

## Overview

Implement the IDAM core as a GoTrue proxy: same API surface as the reference spec (token, user, verify, signup, factors, etc.), with optional server-side session store (e.g. Redis). The implementation can be BRRTRouter-generated from the reference OpenAPI or a shared library that IDAM services depend on.

## Scope

- **IDAM core service:** Skeleton (BRRTRouter codegen from reference spec or shared library); config for GoTrue base URL and path prefix.
- **GoTrue client:** Integrate with Supabase GoTrue (all core paths from [IDAM GoTrue API Mapping](../../../IDAM_GOTRUE_API_MAPPING.md) §3.1).
- **Optional:** Session/Redis store for server-side session state.

## Stories

| Story | Title | Doc | GitHub issue |
|-------|--------|-----|--------------|
| 7.1 | IDAM core service skeleton | [story-7.1-idam-core-service-skeleton.md](story-7.1-idam-core-service-skeleton.md) | [#284](https://github.com/microscaler/BRRTRouter/issues/284) |
| 7.2 | GoTrue client integration | [story-7.2-gotrue-client-integration.md](story-7.2-gotrue-client-integration.md) | [#285](https://github.com/microscaler/BRRTRouter/issues/285) |
| 7.3 | Optional session/Redis store | [story-7.3-optional-session-redis.md](story-7.3-optional-session-redis.md) | [#286](https://github.com/microscaler/BRRTRouter/issues/286) |

## References

- [IDAM GoTrue API Mapping](../../../IDAM_GOTRUE_API_MAPPING.md) §3.1
- [IDAM Design: Core and Extension](../../../IDAM_DESIGN_CORE_AND_EXTENSION.md)
