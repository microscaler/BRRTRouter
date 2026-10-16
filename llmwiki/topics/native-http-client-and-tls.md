---
title: Native HTTP Client and TLS Policy
status: verified
updated: 2026-10-16
---

# Native HTTP client and TLS policy

## Decision

BRRTRouter production code uses `may_minihttp::client::HttpClient` for outbound HTTP/1.1. Both
`http://` and `https://` take this path. HTTPS is implemented inside `may_minihttp`, so consumers do
not maintain separate TLS handshakes or HTTP response parsers.

`reqwest` is a development dependency only. It must not be introduced under `src/` or as a normal
dependency.

## TLS profile

- rustls 0.23 is built with default features disabled and the `ring`, `std`, and `tls12` features.
- AWS-LC is not selected by the production graph.
- The default client configuration verifies certificates and server names against operating-system
  trust through `rustls-platform-verifier`.
- `HttpClient::from_url_with_tls_config` accepts an injected `Arc<rustls::ClientConfig>` for private
  CAs, mTLS, and deterministic local-CA tests.

## URL and request behavior

`HttpClient::from_url` requires an absolute URL, accepts only HTTP and HTTPS, selects port 80 or 443
when omitted, connects using `may::net::TcpStream`, and retains the URL authority as the default
`Host` header. Requests use an origin-form path and query. Plain HTTP does not initialize the TLS
provider or platform verifier.

### `Host` header

HTTP/1.1 requires exactly one `Host` per request; Go's `net/http`, nginx, Envoy and Google front
ends answer `400` without it. Where `Host` comes from depends on how the client was built:

| Client | `Host` on each request |
|--------|------------------------|
| `HttpClient::from_url` / `from_url_with_tls_config` (all of `src/http/fetch.rs`) | The URL authority, inserted by `new_request`. A caller's `extra_headers` `Host` replaces it. |
| `HttpClient::connect(addr)` (`src/http/proxy.rs`) | **None.** The target is origin-form and `may_minihttp` only writes `Host` itself for an absolute-URI target. The caller must set it. |

`proxy_untyped` therefore drops the client's `Host` (it names the BFF) and sets the downstream's own,
built with `brrtrouter::http::host_header_value(host, port, default_port)`:
`<service>.<POD_NAMESPACE>.svc.cluster.local:<SERVICE_HTTP_PORT>`, port omitted when it is the scheme
default, IPv6 literals bracketed. Before 2026-10-16 the proxy sent no `Host`; BRRTRouter downstreams
tolerated that, but the PriceWhisperer platform BFF's proxy to Google Managed Prometheus's query
frontend failed with `400 Bad Request: missing required Host header`.

Any new outbound path built on `HttpClient::connect` must set `Host` the same way.

The delivered client does not implicitly follow redirects, discover proxies, negotiate HTTP/2, or
encode multipart bodies. Those behaviors must be explicit at a caller boundary.

## Why any tests still use reqwest

Most protocol and security tests should use the production client because this catches differences
in parsing, headers, timeout handling, TLS, and coroutine scheduling. Remaining direct uses are
narrow exceptions:

- `tests/common/pet_store_e2e.rs` and `tests/ui_scenarios_pet_store.rs` drive Docker UI scenarios.
  The harness currently relies on separate connect/total deadlines and reqwest multipart encoding.
- `examples/adaptive_load_test.rs` runs inside the async Goose harness and uses its async ecosystem
  for Prometheus queries. Goose itself also carries reqwest transitively.

The disadvantages of using `HttpClient` in those cases today are missing multipart convenience,
no separately configurable connect deadline, and an impedance mismatch with an async load driver.
These are test-harness concerns, not permission for production use. Ordinary readiness probes and
JWKS/security integration tests use `brrtrouter::http`.

## Verification

Run with the current `may_minihttp` branch resolved:

```bash
cargo tree -p brrtrouter -e normal -i reqwest@0.13.2
cargo tree -p brrtrouter -e normal -i aws-lc-rs
cargo test -p brrtrouter --test http_fetch_tests --test jwks_p0_hardening_tests
cargo test -p brrtrouter --lib http::   # host_header_tests + proxy Host capture tests
```

Both tree queries must report `nothing to print`. HTTPS transport is covered in `may_minihttp` by a
local rustls server and generated local CA; no public network is required.

## Code anchors

- `src/http/fetch.rs` — bounded HTTP/HTTPS GET and POST integration.
- `src/http/proxy.rs` — downstream BFF proxy use of the native client; sets the downstream `Host`.
- `src/http/mod.rs` — `host_header_value` (RFC 9110 `Host` value).
- `tests/http_fetch_tests.rs` — `fetch_*_sends_one_host_naming_the_url_authority`,
  `fetch_get_http_extra_host_header_replaces_the_default`.
- `may_minihttp/src/client/client_impl.rs` — URL parsing, plain/TLS transport selection, default `Host`.
- `may_minihttp/src/client/request.rs` — `Host` written only for absolute-URI targets.
- `may_minihttp/Cargo.toml` — explicit rustls provider and feature boundary.
