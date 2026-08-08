# BRRTRouter Roadmap

> **Status (2026-08):** This document is an **archive pointer**, not the live backlog.
> Active work is tracked on epic build boards under [`docs/EPICS/`](EPICS/EPICS_CATALOG.md).
>
> - **Now:** [Epic 12 — Framework maturity](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
> - **Shipped recently:** [Epic 10 URI / request-target](EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md),
>   [Epic 11 HTTP QUERY](EPICS/URI_REQUEST_TARGET/epic-11-http-query-method/README.md)
> - **Parked:** WebSocket (no `may_minihttp` upgrade), radix rewrite, stack-size plumbing as product APIs —
>   see Epic 12 parked list.

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

See **[Epic 12 BUILD_BOARD](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)** (waves 0–4):

- OpenAPI `$ref` requestBodies / responses / pathItems
- Pre-handler query/header validation
- Webhook outbound delivery kit
- Multipart form-data truth
- Multi-status typed/codegen
- Perf science (Phase 6)

Also: crates.io packaging polish; fake OTEL collector coverage in remaining tests.

## ⏸ Parked (explicit non-goals for now)

- Native WebSocket upgrade in-process
- Radix trie rewrite “for its own sake”
- Full OAS callback auto-fire runtime
- Fleet-wide OpenAPI 3.2.0 cutover (products stay 3.1; see [OPENAPI_VERSION_SUPPORT.md](OPENAPI_VERSION_SUPPORT.md))

## 🎯 Benchmark Goal

- Cloud-native scale-out (≤2 cores / 500 Mi typical pod)
- Evidence-driven optimization via Epic 12.8 — not premature radix rewrites

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
- [Epic 12 build board](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
- [OpenAPI version support](OPENAPI_VERSION_SUPPORT.md)
- [Describing API Security](https://learn.openapis.org/specification/security.html)
