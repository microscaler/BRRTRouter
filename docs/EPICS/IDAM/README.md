# IDAM Epics and Stories

**Source analysis:** [docs/IDAM_MICROSCALER_ANALYSIS.md](../../IDAM_MICROSCALER_ANALYSIS.md), [docs/IDAM_GOTRUE_API_MAPPING.md](../../IDAM_GOTRUE_API_MAPPING.md), [docs/IDAM_DESIGN_CORE_AND_EXTENSION.md](../../IDAM_DESIGN_CORE_AND_EXTENSION.md)

**Summary (for authoring and GitHub issues):** [EPICS_AND_STORIES_SUMMARY.md](EPICS_AND_STORIES_SUMMARY.md)

This directory contains Epics and Stories for implementing the IDAM pattern in Microscaler/BRRTRouter: shared core (GoTrue proxy + optional session) and per-system extensions (API keys, preferences, domain model); BFF uses a single IDAM base URL.

**Numbering:** IDAM epics continue from BFF_PROXY (Epics 1–5). See [EPICS_CATALOG.md](../EPICS_CATALOG.md).

## Epics

| Epic | Title | Directory | GitHub issue |
|------|--------|-----------|--------------|
| 6 | IDAM contract and reference spec | [epic-6-idam-contract/](epic-6-idam-contract/) | [#278](https://github.com/microscaler/BRRTRouter/issues/278) |
| 7 | IDAM core implementation (GoTrue proxy) | [epic-7-idam-core/](epic-7-idam-core/) | [#279](https://github.com/microscaler/BRRTRouter/issues/279) |
| 8 | IDAM extension and build/deploy | [epic-8-idam-extension/](epic-8-idam-extension/) | [#280](https://github.com/microscaler/BRRTRouter/issues/280) |
| 9 | BFF ↔ IDAM integration | [epic-9-bff-idam/](epic-9-bff-idam/) | [#281](https://github.com/microscaler/BRRTRouter/issues/281) |

Each Epic has its own subdirectory with an Overview README and story files. Epic and Story docs reference the corresponding GitHub issue numbers.

## Quick reference

- **Epic 6:** Document IDAM contract (endpoints and behaviour expected by BFF); reference IDAM core OpenAPI derived from GoTrue.
- **Epic 7:** IDAM core service (GoTrue proxy, optional session/Redis); BRRTRouter-generated or shared library.
- **Epic 8:** Core + extension spec merge at build (single service); path conventions and ingress rules (two-service option).
- **Epic 9:** BFF IDAM base URL config, path layout, and documentation for BFF usage of IDAM.

## References

- [IDAM Microscaler Analysis](../../IDAM_MICROSCALER_ANALYSIS.md)
- [IDAM GoTrue API Mapping](../../IDAM_GOTRUE_API_MAPPING.md)
- [IDAM Design: Core and Extension](../../IDAM_DESIGN_CORE_AND_EXTENSION.md)
- [BFF Proxy Analysis](../../BFF_PROXY_ANALYSIS.md) §5.4, §6
- [Epics catalog (1–9)](../EPICS_CATALOG.md)
