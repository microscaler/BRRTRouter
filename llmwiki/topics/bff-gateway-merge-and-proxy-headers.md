# BFF gateway path merge and proxy response headers

- **Status**: `active`
- **Code anchors**: `tooling/src/brrtrouter_tooling/bff/merge.py`, `tooling/src/brrtrouter_tooling/helpers.py`, `tooling/src/brrtrouter_tooling/workspace/bff/generate_system.py`, `src/http/proxy.rs`, `src/server/response.rs`
- **Sibling**: Hauliage [`docs/llmwiki/topics/k8s-native-bff-routing.md`](../../../../hauliage/docs/llmwiki/topics/k8s-native-bff-routing.md)
- **Last updated**: 2026-10-16

## Gateway public paths (suite config merge)

Hauliage frontend calls **product paths** such as `/api/v1/bidding/quotes` and `/api/v1/consignments/jobs`. Sub-service OpenAPI specs often declare **resource-only** paths (`/quotes`, `/jobs`).

The BFF merge pipeline supports `gateway_path_style` per service in `openapi/bff-suite-config.yaml`:

| Style | Example base | Spec path | BFF path key |
|-------|--------------|-----------|--------------|
| `prefixed` (default for most services) | `/api/v1/bidding` | `/quotes` | `/bidding/quotes` |
| `as_spec` (company, marketing) | `/api/v1/company` | `/organizations/me` | `/organizations/me` |

**Implementation**

- `gateway_public_path()` / `normalize_spec_path()` in `helpers.py`
- `merge_sub_service_specs()` in `bff/merge.py` registers gateway path keys; `x-brrtrouter-downstream-path` remains the full downstream URL (`base_path + resource path`).
- `hauliage bff generate-system` **must** delegate to `bff-suite-config.yaml` when present (`workspace/bff/generate_system.py`). Legacy directory discovery alone leaves bare `/quotes` routes and breaks the frontend.

**Merged spec `servers`**

BRRTRouter route registration uses **`servers[0].url` as `base_path`**. Hauliage BFF output must set:

```yaml
servers:
  - url: /api/v1
```

Do not use bare `http://localhost:8080` as the first server — routes would register without the `/api/v1` prefix.

## Proxy response headers (duplicate Content-Length)

BFF downstream proxy (`proxy_untyped`) forwards downstream response headers into `HandlerResponse`. `write_handler_response` then serializes JSON via `may_minihttp::Response::body_vec`, which **adds its own `Content-Length`**.

If downstream `Content-Length` is forwarded, clients see **duplicate** `Content-Length` headers. **curl** may tolerate this; **Node.js** (Vite dev proxy) fails with `Parse Error: Duplicate Content-Length` → HTTP 502.

**Fix (2026-07-07)**

1. `skip_forward_response_header()` in `proxy.rs` — do not forward `Content-Length` (or `Transfer-Encoding`).
2. `write_handler_response()` — skip any handler-supplied `content-length` before `body_vec()`.

**Tests**

- Rust: `server::response::tests::test_write_handler_response_ignores_incoming_content_length`
- Rust: `http::proxy::tests::skip_forward_response_header_blocks_hop_by_hop`
- Python: `TestGatewayPublicPath`, `test_merge_prefixed_gateway_paths`, `test_suite_config_uses_prefixed_gateway_paths`

Run Rust tests on **ms02** (Mac cross-compile may fail on `ring`/CPU features).

## Proxy request `Host` (2026-10-16)

`proxy_untyped` connects with `HttpClient::connect(addr)` and sends an origin-form target, so
`may_minihttp` adds no `Host`; the client's `Host` (the BFF's public name) is deliberately not
forwarded (`skip_forward_request_header`). Until this fix the downstream got **no** `Host`, which
strict HTTP/1.1 servers reject with 400 (seen against Google Managed Prometheus's Go frontend).

The proxy now inserts exactly one `Host` naming the downstream:
`host_header_value(downstream_host(service), downstream_http_port(), 80)` →
`<service>.<ns>.svc.cluster.local:8080`. Details: [native HTTP client](./native-http-client-and-tls.md#host-header).

**Tests**

- `http::proxy::tests::proxy_sends_the_downstream_host_not_the_clients`
- `http::proxy::tests::proxy_sends_the_downstream_host_when_the_client_sent_none`
- `http::proxy::tests::downstream_host_header_for_a_namespaced_service`
- `http::host_header_tests::*`
