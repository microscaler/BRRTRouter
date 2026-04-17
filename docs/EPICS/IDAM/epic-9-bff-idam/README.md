# Epic 9 — BFF ↔ IDAM integration

**GitHub issue:** [#281](https://github.com/microscaler/BRRTRouter/issues/281)

**Catalog:** [Epics 1–9](../EPICS_CATALOG.md) | **Theme:** IDAM (Epics 6–9)

## Overview

Ensure the BFF uses a single IDAM base URL for all IDAM calls (auth, users/me, preferences, API keys); document config and path layout. Document how BFF uses IDAM for claims enrichment and RBAC (link to BFF Proxy Analysis §5.4, §6).

## Scope

- **BFF config:** `IDAM_BASE_URL` (and optional `IDAM_JWKS_URL`, `IDAM_ISSUER`); path layout so BFF and IDAM agree on paths.
- **Documentation:** How BFF uses IDAM: single URL, claims enrichment (call IDAM users/me or introspect), RBAC from JWT or IDAM API; cross-link to BFF Proxy Analysis and IDAM contract (Epic 6).

## Stories

| Story | Title | Doc | GitHub issue |
|-------|--------|-----|--------------|
| 9.1 | BFF IDAM base URL config and path layout | [story-9.1-bff-idam-base-url-config.md](story-9.1-bff-idam-base-url-config.md) | [#289](https://github.com/microscaler/BRRTRouter/issues/289) |
| 9.2 | Document BFF usage of IDAM | [story-9.2-document-bff-usage-idam.md](story-9.2-document-bff-usage-idam.md) | [#290](https://github.com/microscaler/BRRTRouter/issues/290) |

## References

- [BFF Proxy Analysis](../../../BFF_PROXY_ANALYSIS.md) §5.4, §6
- [IDAM Design: Core and Extension](../../../IDAM_DESIGN_CORE_AND_EXTENSION.md) §4
- [Epic 6 — IDAM contract](../epic-6-idam-contract/README.md)
