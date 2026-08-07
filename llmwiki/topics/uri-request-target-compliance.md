# URI / request-target compliance (Epics 10–11)

- **Status:** partially-verified (encode fix shipped; epic matrix not yet Pass)
- **Source docs:**
  - [`docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md`](../../docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md)
  - [`docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`](../../docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md)
  - [`docs/EPICS/URI_REQUEST_TARGET/`](../../docs/EPICS/URI_REQUEST_TARGET/)
  - [`docs/EPICS/URI_REQUEST_TARGET/TESTING_STANDARD.md`](../../docs/EPICS/URI_REQUEST_TARGET/TESTING_STANDARD.md)
  - [`docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md`](../../docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md)
- **Code anchors:**
  - `src/server/request.rs` — `parse_query_params`, `parse_request`, `decode_param_value`
  - `src/http/proxy.rs` — `resolve_path_template`, `proxy_untyped`, `ProxyError`
  - `src/router/` — path template / path params
  - may_minihttp (sibling) — request-line → path string

## What happened (2026-08-07)

Loadlinker BFF returned **502** `invalid path: invalid uri character` for
`GET /api/v1/locations/provinces?country=South%20Africa`. Inbound decode (WHATWG
form-urlencoded) correctly produced a space; `resolve_path_template` appended
the decoded value **without** re-encoding → `http::Uri` reject →
`ProxyError::InvalidPath` mapped to **502**.

**Shipped fix:** percent-encode path/query components before URI composition
(`urlencoding::encode` in `resolve_path_template`). Unit tests cover spaces,
accents, reserved delimiters, and negative Uri-fail cases.

## Intentional asymmetry

| Direction | Mechanism | Space |
|-----------|-----------|-------|
| Inbound query | `url::form_urlencoded` | `+` and `%20` → space |
| Outbound rebuild | URI component encode | space → `%20` (not `+`) |

## Epics

| Epic | Issue | Scope | Build board |
|------|-------|--------|-------------|
| **10** | [#373](https://github.com/microscaler/BRRTRouter/issues/373) | 100% request-target parse/rebuild (RFC 3986 / 9110 / form-urlencoded) | [BUILD_BOARD.md](../../docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md) |
| **11** | [#374](https://github.com/microscaler/BRRTRouter/issues/374) | HTTP **QUERY** method (RFC 10008) — body query; **not** a substitute for Epic 10 | same board (after 10.1; 11.3 wants 10.7) |

## Testing mandate

Every story must ship comprehensive **positive and negative** unit tests before
Done — see [`TESTING_STANDARD.md`](../../docs/EPICS/URI_REQUEST_TARGET/TESTING_STANDARD.md)
(minima ≥5 each; no panic on hostile input; named `*_positive_*` / `*_negative_*`).

## Gaps / drift (open)

- Compliance matrix + goldens **shipped** (Story 10.1 / [#375](https://github.com/microscaler/BRRTRouter/issues/375)):
  `docs/EPICS/URI_REQUEST_TARGET/compliance-matrix.md`, `tests/uri_golden/`, `tests/uri_golden_harness.rs`.
- `ProxyError` still maps composition failures to **502** (Story 10.7).
- Single `urlencoding::encode` site — Story 10.4 wants named path vs query APIs.
- Dual `http` 1.0 vs `http_legacy` 0.2 URI stacks (Story 10.8).
- QUERY method not first-class (Epic 11).
- RFC 10008 is **orthogonal** to percent-encoding; do not conflate in fixes.

## Build order (summary)

```text
Wave 0: 10.1
Wave 1: 10.2 ‖ 10.3 ‖ 10.11
Wave 2: 10.4
Wave 3: 10.5 ‖ 10.6
Wave 4: 10.7
Wave 5: 10.9 → 10.10 → 10.8
Epic 11: 11.1 → 11.2 → 11.3 → 11.4  (after 10.1; 11.3 after 10.7)
```

**Waves 0–2 done.** **NOW:** Wave 3 — **10.5** passthrough
([#379](https://github.com/microscaler/BRRTRouter/issues/379)) ‖ **10.6** 414
([#380](https://github.com/microscaler/BRRTRouter/issues/380)).
Encoders: `http::uri_encode`. Boundary: [`request-line-boundary.md`](../../docs/EPICS/URI_REQUEST_TARGET/request-line-boundary.md).
Full index: [`BUILD_BOARD.md`](../../docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md).
