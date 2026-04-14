# Epics catalog

**Purpose:** Single catalog of all Epics (global numbering). BFF Proxy = Epics 1–5; IDAM = Epics 6–9.

## All epics (1–9)

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

**GitHub metadata mapping:**
- **Labels** → Theme + role: `bff-proxy` or `idam`; epics also get `epic`, stories get `story`.
- **Type** → Issue type: Epic = Feature / Task; Story = story (set in GitHub UI if org has issue types).
- **Relationships** → Parent: each story is a sub-issue of its Epic (Story N.M has parent Epic #); use GitHub Relationships “Add parent” or MCP `sub_issue_write`. Blocked-by can link stories that depend on others.

## By theme

- **BFF_PROXY** (Epics 1–5): [BFF_PROXY/README.md](BFF_PROXY/README.md)
- **IDAM** (Epics 6–9): [IDAM/README.md](IDAM/README.md)
