# Epics catalog

**Purpose:** Single catalog of all Epics (global numbering).

| Range | Theme |
|-------|--------|
| 1–5 | BFF Proxy |
| 6–9 | IDAM |
| 10–11 | URI / request-target |
| 12–13 | Framework maturity / completeness |
| 14 | Zero-trust SPIFFE / mTLS |
| 15 | OpenAPI surface |
| 16 | Release & observability maturity |

## All epics (1–16)

| Epic | Title | Theme (Labels) | Type | Directory | GitHub issue |
|------|--------|----------------|------|-----------|--------------|
| 1 | Spec-driven proxy (RouteMeta + BFF generator) | BFF_PROXY, epic | Feature | [BFF_PROXY/epic-1-spec-driven-proxy/](BFF_PROXY/epic-1-spec-driven-proxy/) | [#254](https://github.com/microscaler/BRRTRouter/issues/254) |
| 2 | BFF proxy library and generated handlers | BFF_PROXY, epic | Feature | [BFF_PROXY/epic-2-proxy-library/](BFF_PROXY/epic-2-proxy-library/) | [#255](https://github.com/microscaler/BRRTRouter/issues/255) |
| 3 | BFF ↔ IDAM auth/RBAC | BFF_PROXY, epic | Feature | [BFF_PROXY/epic-3-bff-idam-auth/](BFF_PROXY/epic-3-bff-idam-auth/) | [#256](https://github.com/microscaler/BRRTRouter/issues/256) |
| 4 | Enrich downstream with claims/RBAC | BFF_PROXY, epic | Feature | [BFF_PROXY/epic-4-enrich-downstream/](BFF_PROXY/epic-4-enrich-downstream/) | [#257](https://github.com/microscaler/BRRTRouter/issues/257) |
| 5 | Microservices: claims in handlers + Lifeguard row-based access | BFF_PROXY, epic | Feature | [BFF_PROXY/epic-5-microservices-claims-lifeguard/](BFF_PROXY/epic-5-microservices-claims-lifeguard/) | [#258](https://github.com/microscaler/BRRTRouter/issues/258) |
| 6 | IDAM contract and reference spec | IDAM, epic | Feature | [IDAM/epic-6-idam-contract/](IDAM/epic-6-idam-contract/) | [#278](https://github.com/microscaler/BRRTRouter/issues/278) |
| 7 | IDAM core implementation (GoTrue proxy) | IDAM, epic | Feature | [IDAM/epic-7-idam-core/](IDAM/epic-7-idam-core/) | [#279](https://github.com/microscaler/BRRTRouter/issues/279) |
| 8 | IDAM extension and build/deploy | IDAM, epic | Feature | [IDAM/epic-8-idam-extension/](IDAM/epic-8-idam-extension/) | [#280](https://github.com/microscaler/BRRTRouter/issues/280) |
| 9 | BFF ↔ IDAM integration | IDAM, epic | Feature | [IDAM/epic-9-bff-idam/](IDAM/epic-9-bff-idam/) | [#281](https://github.com/microscaler/BRRTRouter/issues/281) |
| 10 | Request-target parse & rebuild — 100% URI compliance | URI_REQUEST_TARGET, epic | Feature | [URI_REQUEST_TARGET/epic-10-uri-request-target-compliance/](URI_REQUEST_TARGET/epic-10-uri-request-target-compliance/) | [#373](https://github.com/microscaler/BRRTRouter/issues/373) |
| 11 | HTTP QUERY method (RFC 10008) | URI_REQUEST_TARGET, epic | Feature | [URI_REQUEST_TARGET/epic-11-http-query-method/](URI_REQUEST_TARGET/epic-11-http-query-method/) | [#374](https://github.com/microscaler/BRRTRouter/issues/374) |
| 12 | Framework maturity — safety, OpenAPI fidelity, kits | FRAMEWORK_MATURITY, epic | Feature | [FRAMEWORK_MATURITY/epic-12-framework-maturity/](FRAMEWORK_MATURITY/epic-12-framework-maturity/) | [#391](https://github.com/microscaler/BRRTRouter/issues/391) |
| 13 | Framework completeness — ops, errors, files, DevEx | FRAMEWORK_MATURITY, epic | Feature | [FRAMEWORK_MATURITY/epic-13-framework-completeness/](FRAMEWORK_MATURITY/epic-13-framework-completeness/) | [#400](https://github.com/microscaler/BRRTRouter/issues/400) |
| 14 | SPIFFE X.509 / mTLS / Federation | ZERO_TRUST, epic | Feature | [ZERO_TRUST/epic-14-spiffe-mtls-federation/](ZERO_TRUST/epic-14-spiffe-mtls-federation/) | [#411](https://github.com/microscaler/BRRTRouter/issues/411) |
| 15 | OpenAPI surface completion | OPENAPI_SURFACE, epic | Feature | [OPENAPI_SURFACE/epic-15-openapi-surface-completion/](OPENAPI_SURFACE/epic-15-openapi-surface-completion/) | [#412](https://github.com/microscaler/BRRTRouter/issues/412) |
| 16 | Release & observability maturity | RELEASE_MATURITY, epic | Feature | [RELEASE_MATURITY/epic-16-release-and-observability/](RELEASE_MATURITY/epic-16-release-and-observability/) | [#413](https://github.com/microscaler/BRRTRouter/issues/413) |

**GitHub metadata mapping:**
- **Labels** → Theme + role: `bff-proxy`, `idam`, `uri-request-target`, `framework-maturity`, `zero-trust`, `openapi-surface`, `release-maturity`; epics also get `epic`, stories get `story`.
- **Relationships** → Each story is a sub-issue of its Epic parent.

## By theme

- **BFF_PROXY** (Epics 1–5): [BFF_PROXY/README.md](BFF_PROXY/README.md)
- **IDAM** (Epics 6–9): [IDAM/README.md](IDAM/README.md)
- **URI_REQUEST_TARGET** (Epics 10–11): [URI_REQUEST_TARGET/README.md](URI_REQUEST_TARGET/README.md)
- **FRAMEWORK_MATURITY** (Epics 12–13): [FRAMEWORK_MATURITY/README.md](FRAMEWORK_MATURITY/README.md) · [BUILD_BOARD.md](FRAMEWORK_MATURITY/BUILD_BOARD.md)
- **ZERO_TRUST** (Epic 14): [ZERO_TRUST/README.md](ZERO_TRUST/README.md) · [BUILD_BOARD.md](ZERO_TRUST/BUILD_BOARD.md) — **critical mTLS**
- **OPENAPI_SURFACE** (Epic 15): [OPENAPI_SURFACE/README.md](OPENAPI_SURFACE/README.md) · [BUILD_BOARD.md](OPENAPI_SURFACE/BUILD_BOARD.md)
- **RELEASE_MATURITY** (Epic 16): [RELEASE_MATURITY/README.md](RELEASE_MATURITY/README.md) · [BUILD_BOARD.md](RELEASE_MATURITY/BUILD_BOARD.md)

**Parked (no epic):** [PARKED.md](PARKED.md) — WebSocket, callback auto-fire engine, radix rewrite.
