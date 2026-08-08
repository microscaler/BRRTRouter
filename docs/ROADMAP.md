# BRRTRouter Roadmap

> **Status (2026-08):** This document is an **archive pointer**, not the live backlog.
> Active work is tracked on epic build boards under [`docs/EPICS/`](EPICS/EPICS_CATALOG.md).
>
> - **Now:** [Epic 13 — Framework completeness](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
> - **Shipped recently:** [Epic 12 Framework maturity](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/README.md) (12.1–12.8),
>   [Epic 10 URI / request-target](EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md),
>   [Epic 11 HTTP QUERY](EPICS/URI_REQUEST_TARGET/epic-11-http-query-method/README.md)
> - **Parked:** WebSocket (no `may_minihttp` upgrade), radix rewrite, OAS callback auto-fire,
>   SPIFFE X.509/mTLS — see Epic 13 parked list.
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
- Hot reload, SSE (`x-sse`, buffered), stack sizing ([stack_size.md](stack_size.md))
- Inbound body hard caps → 413 ([request_body_limits.md](request_body_limits.md))
- BFF auto-proxy integration; Pet Store example; Tilt + kind local stack

## 🚧 Active / next

See **[Epic 13 BUILD_BOARD](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)** (waves 0–5):

- Doc truth & claim reconciliation
- Rate limiting middleware → 429
- Problem Details (RFC 7807)
- Streaming uploads / download helpers
- Browser security posture (kit or explicit OOS)
- Handler deadlines → 504
- SSE live flush; opt-in response compression
- Multi-status codegen + public TestApp

Also: crates.io packaging polish; fake OTEL collector coverage in remaining tests.

## ⏸ Parked (explicit non-goals for now)

- Native WebSocket upgrade in-process
- Radix trie rewrite “for its own sake”
- Full OAS callback auto-fire runtime
- Fleet-wide OpenAPI 3.2.0 cutover (products stay 3.1; see [OPENAPI_VERSION_SUPPORT.md](OPENAPI_VERSION_SUPPORT.md))

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
- [Epic 13 build board](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
- [Epic 12 (done)](EPICS/FRAMEWORK_MATURITY/epic-12-framework-maturity/README.md)
- [OpenAPI version support](OPENAPI_VERSION_SUPPORT.md)
- [Describing API Security](https://learn.openapis.org/specification/security.html)
