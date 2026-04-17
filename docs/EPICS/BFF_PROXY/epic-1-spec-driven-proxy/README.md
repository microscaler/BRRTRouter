# Epic 1 — Spec-driven proxy (RouteMeta + BFF generator)

**GitHub issue:** [#254](https://github.com/microscaler/BRRTRouter/issues/254)

## Overview

Enable proxy target to be defined in the BFF OpenAPI spec and consumed by BRRTRouter so that downstream path and service key are the single source of truth. The BFF generator emits operation-level extensions; BRRTRouter reads them into `RouteMeta`. Also ensure the BFF generator merges components and security so the BFF spec is OpenAPI 3.1 compliant and BRRTRouter auth works.

## Scope

- **BRRTRouter:** Add `downstream_path` and `x_service` (or equivalent) to `RouteMeta` in `spec/types.rs`; populate from operation extensions in `spec/build.rs` (reading `x-brrtrouter-downstream-path` and `x-service`).
- **BFF generator (e.g. RERP `generate_system.py` or bff-generator):** Emit `x-brrtrouter-downstream-path` (exact path on downstream) and `x-service` (service key) per operation; merge `components.parameters`, `components.securitySchemes`, and root `security` (OPENAPI_3.1.0_COMPLIANCE_GAP §8).

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 1.1 | RouteMeta extensions in BRRTRouter | [story-1.1-route-meta-extensions.md](story-1.1-route-meta-extensions.md) |
| 1.2 | BFF generator proxy extensions | [story-1.2-bff-generator-proxy-extensions.md](story-1.2-bff-generator-proxy-extensions.md) |
| 1.3 | BFF generator components/security merge | [story-1.3-bff-generator-components-security.md](story-1.3-bff-generator-components-security.md) |
| 1.4 | Extract BFF generator to BRRTRouter tooling | [story-1.4-extract-bff-tooling-to-brrrouter.md](story-1.4-extract-bff-tooling-to-brrrouter.md) |

## References

- `docs/BFF_PROXY_ANALYSIS.md` §3.2, §4 (G2, G6), §5.2, §5.6
- BRRTRouter: `src/spec/types.rs`, `src/spec/build.rs`
- OPENAPI_3.1.0_COMPLIANCE_GAP.md §8
- **BFF generator location:** [BFF_GENERATOR_EXTRACTION_ANALYSIS.md](../BFF_GENERATOR_EXTRACTION_ANALYSIS.md) — whether to extract the BFF generator into BRRTRouter tooling so any consumer has standard BFF tools; RERP can import the module or call the CLI.
