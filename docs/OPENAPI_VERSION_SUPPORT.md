# OpenAPI version support (3.1 + 3.2 dual contract)

BRRTRouter loads OpenAPI documents through `oas3` **0.21** (3.1.x types) plus a
preprocess step for RFC 10008 **QUERY**. This page is the contract for product
suites (hauliage, sesame-idam, rerp, PriceWhisperer, idam).

## Supported version strings

| `openapi:` field | Loader behaviour | Notes |
|------------------|------------------|--------|
| **3.1.0** / **3.1.x** | Full current path | Fleet default. Prefer staying here. |
| **3.1.2** | Same as 3.1.0 | Doc hygiene only — **does not** add Path Item `query`. |
| **3.2.0** / **3.2.x** | Accepted for the **supported subset** below | Version string alone ≠ full OAS 3.2. |
| **3.0.x** | Best-effort / legacy | Prefer migrate to 3.1.0. |

BRRTRouter does **not** call `oas3::validate_version`; a `3.2.0` document is not
rejected solely for its version string.

## QUERY (RFC 10008) — works on 3.1 and 3.2

OAS **3.1.x** Path Item has no native `query` field. OAS **3.2.0** does
([Path Item Object](https://spec.openapis.org/oas/v3.2.0#path-item-object)).

Until `oas3` exposes `PathItem::query` ([oas3-rs#300](https://github.com/x52dev/oas3-rs/issues/300)),
BRRTRouter always:

1. Promotes path-level `query:` / `QUERY` → `x-brrtrouter-query` ([`promote_query_operations`](../src/spec/load.rs))
2. Registers routes with [`method_query()`](../src/http/method_ext.rs)

So you may declare `query:` on **`openapi: 3.1.0`** specs today — **no need to
bump the version string to 3.2.0** just for QUERY.

See also: [declaring-query-operations.md](./EPICS/URI_REQUEST_TARGET/epic-11-http-query-method/declaring-query-operations.md).

## OAS 3.2 feature matrix (BRRTRouter)

| 3.2 feature | Status |
|-------------|--------|
| Path Item `query` | **Supported** via promote → `x-brrtrouter-query` (native field when oas3 ships it) |
| `x-brrtrouter-query` | **Supported** escape hatch (3.1 tooling that strips bare `query:`) |
| `additionalOperations` | **Unsupported** — stripped by `strip_unknown_verbs` (fail-closed: not registered) |
| `in: querystring` | **Unsupported** — `oas3` deserialize fails; do not use |
| `itemSchema` / sequential media / encoding deltas | **Ignored** (dropped by parser); SSE remains `x-sse` |
| Hierarchical tags / `Server.name` / etc. | **Ignored** if present |

“3.2 ready” for a **fleet** cutover means: this matrix is updated, `oas3` has
native 3.2 types for the features you use, and release notes say so. Until then,
**do not** mass-bump product `openapi:` to `3.2.0`.

## Product suite policy (hauliage, sesame-idam, rerp, PriceWhisperer, idam)

1. Keep canonical specs at **`openapi: 3.1.0`**.
2. When adding RFC 10008 search, add path-item **`query:`** (or `x-brrtrouter-query`)
   without changing the version string.
3. Bump BRRTRouter git/path pin after QUERY-capable releases; do **not** bump
   suite `openapi:` to 3.2.0 until BRRTRouter release notes declare the 3.2
   subset you need.
4. Leave `3.0.3` holdouts as a separate cleanup → 3.1.0 (unrelated to QUERY).

## Upstream tracking

| Item | Link |
|------|------|
| `oas3` OpenAPI 3.2 support | https://github.com/x52dev/oas3-rs/issues/300 |
| Prefer native `item.query` then extension | [`src/spec/build.rs`](../src/spec/build.rs) — `query_operation_from_path_item` |
| Fixture proving `openapi: 3.2.0` + `query:` loads | [`tests/fixtures/openapi_query_method_32.yaml`](../tests/fixtures/openapi_query_method_32.yaml) |

When upgrading `oas3`, evaluate 0.22+ MSRV / schema churn separately from 3.2 types.
