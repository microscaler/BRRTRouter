# Epic 8 — IDAM extension and build/deploy

**GitHub issue:** [#280](https://github.com/microscaler/BRRTRouter/issues/280)

**Catalog:** [Epics 1–9](../EPICS_CATALOG.md) | **Theme:** IDAM (Epics 6–9)

## Overview

Support IDAM core + extension: (1) merge reference core and customer extension OpenAPI at build time so a single IDAM service serves both; (2) document path conventions and ingress rules for the two-service deployment option (core + extension on two ports, ingress path-based routing).

## Scope

- **Merged spec:** Generator or manual step merges `idam-core.openapi.yaml` and customer `idam-extension.openapi.yaml` into one combined spec; BRRTRouter codegen on combined spec → single IDAM binary.
- **Path conventions:** Document core vs extension path prefixes so ingress rules (for two-service option) are unambiguous (see [IDAM Design: Core and Extension](../../../IDAM_DESIGN_CORE_AND_EXTENSION.md) §4.2).

## Stories

| Story | Title | Doc | GitHub issue |
|-------|--------|-----|--------------|
| 8.1 | Core + extension spec merge at build | [story-8.1-core-extension-spec-merge.md](story-8.1-core-extension-spec-merge.md) | [#287](https://github.com/microscaler/BRRTRouter/issues/287) |
| 8.2 | Path conventions and ingress rules | [story-8.2-path-conventions-ingress.md](story-8.2-path-conventions-ingress.md) | [#288](https://github.com/microscaler/BRRTRouter/issues/288) |

## References

- [IDAM Design: Core and Extension](../../../IDAM_DESIGN_CORE_AND_EXTENSION.md) §2, §4.2
- [IDAM GoTrue API Mapping](../../../IDAM_GOTRUE_API_MAPPING.md) §3.2
