# BRRTRouter Roadmap

> **Status (2026-08):** This document is an **archive pointer**, not the live backlog.
> Active work is tracked on epic build boards under [`docs/EPICS/`](EPICS/EPICS_CATALOG.md).
>
> - **Now (parallel themes):**
>   - [Epic 13 — Framework completeness](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
>   - [Epic 14 — SPIFFE X.509 / mTLS / Federation](EPICS/ZERO_TRUST/BUILD_BOARD.md) (**critical**)
>   - [Epic 15 — OpenAPI surface](EPICS/OPENAPI_SURFACE/BUILD_BOARD.md)
>   - [Epic 16 — Release & observability](EPICS/RELEASE_MATURITY/BUILD_BOARD.md)
> - **Shipped recently:** [Epic 12](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/README.md),
>   [Epic 10](EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md),
>   [Epic 11](EPICS/URI_REQUEST_TARGET/epic-11-http-query-method/README.md)
> - **Parked:** WebSocket, OAS callback auto-fire engine, radix rewrite — [PARKED.md](EPICS/PARKED.md)
> - **Open reference product:** [Sesame-IDAM](https://github.com/microscaler/sesame-idam) —
>   [BUILDING_WITH_BRRTROUTER.md](BUILDING_WITH_BRRTROUTER.md) (not private Hauliage / PriceWhisperer; not immature RERP).

Historical May 2025 notes below are retained for archaeology. Many items marked “Planned” there
are **already shipped** (CORS, metrics, hot reload, typed panic recovery, schema validation, etc.).
Do not treat those bullets as open work.

## ✅ Completed (high level)

- OpenAPI 3.1 specification parser (+ QUERY promote / dual-support policy)
- O(k) radix routing (`PathCursor`); legacy regex table not on the hot path
- Coroutine HTTP server (`may_minihttp` / `may`)
- Dynamic handler dispatch, typed handlers + panic recovery
- Request context (path, query, headers, cookies, JSON body)
- Schema validation, JWT/OAuth2/API-key providers, CORS, Prometheus/OTEL
- Hot reload, SSE (`x-sse`, live flush via `HttpSse`), stack sizing ([stack_size.md](stack_size.md))
- Inbound body hard caps → 413 ([request_body_limits.md](request_body_limits.md))
- BFF auto-proxy integration; Pet Store example; Tilt + kind local stack

## 🚧 Active / next

| Epic | Board | Focus |
|------|--------|--------|
| **13** | [Framework completeness](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) | Rate limit, problem+json, files, browser, deadlines, SSE, compression, TestApp |
| **14** | [Zero-trust SPIFFE/mTLS](EPICS/ZERO_TRUST/BUILD_BOARD.md) | **Critical** X.509 SVID, mTLS path, federation, JWT hardening |
| **15** | [OpenAPI surface](EPICS/OPENAPI_SURFACE/BUILD_BOARD.md) | Headers, servers, encoding, methods, versioning, callbacks fidelity, 3.2 readiness |
| **16** | [Release maturity](EPICS/RELEASE_MATURITY/BUILD_BOARD.md) | API policy, crates.io, fake OTEL, beta checklist, migration guide |

Catalog: [EPICS_CATALOG.md](EPICS/EPICS_CATALOG.md).

## ⏸ Parked (explicit non-goals for now)

See **[EPICS/PARKED.md](EPICS/PARKED.md)**.

- Native WebSocket upgrade in-process
- Radix trie rewrite “for its own sake”
- Full OAS callback auto-fire runtime (object fidelity = Epic 15.7; outbound kit = 12.5)
- Fleet-wide forced OpenAPI 3.2.0 cutover (readiness plan = Epic 15.8)

## 🎯 Benchmark Goal

- Cloud-native scale-out (≤2 cores / 500 Mi typical pod)
- Evidence-driven optimization via Epic 12.8 — not premature radix rewrites
- Epic 13 focuses on e2e/dispatch/ops completeness, not trie rewrites

---

## Archive — May 2025 notes (stale; do not use as backlog)

The following sections are **historical**. Shipped items remain listed for context; open bullets
were either completed later or superseded by Epic boards.

<details>
<summary>Expand archived May 2025 roadmap text</summary>

### Formerly “Planned” (many shipped since)

- REST status from typed handlers — see [PRD_TYPED_HANDLER_HTTP_STATUS.md](PRD_TYPED_HANDLER_HTTP_STATUS.md)
- Config context / `RuntimeConfig`
- Fake otel collector across all tests (still partial)
- Docker compose / Tilt observability stack (shipped via Tilt path)
- Typed handler deserialization / `#[handler]` / hot reload / CORS / metrics / schema validation — **shipped**
- WebSocket support — **parked** (Epic 12)
- Packaging reusable SDKs on crates.io — still open

### Formerly listed task sketches

Task-list prose from May 2025 (server handshake, typed panic catch, CORS, Prometheus, etc.)
described work that is largely landed. Prefer epic story files and GitHub issues over this archive.

</details>

## links

- [Epics catalog](EPICS/EPICS_CATALOG.md)
- [Epics catalog](EPICS/EPICS_CATALOG.md)
- [Epic 13 board](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) · [Epic 14 zero-trust](EPICS/ZERO_TRUST/BUILD_BOARD.md) · [Epic 15 OpenAPI](EPICS/OPENAPI_SURFACE/BUILD_BOARD.md) · [Epic 16 release](EPICS/RELEASE_MATURITY/BUILD_BOARD.md)
- [Epic 12 (done)](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/README.md)
- [Parked](EPICS/PARKED.md)
- [OpenAPI version support](OPENAPI_VERSION_SUPPORT.md)
- [Describing API Security](https://learn.openapis.org/specification/security.html)
