# Epic 2 — BFF proxy library and generated handlers

**GitHub issue:** [#255](https://github.com/microscaler/BRRTRouter/issues/255)

## Overview

Implement the runtime proxy: a library that builds the downstream URL from config + RouteMeta, forwards the HTTP request (method, path, query, headers, body), and returns the backend response. Config maps service key (`x_service`) to base URL. Askama generates a thin proxy handler when RouteMeta has proxy metadata; no per-route URL logic in templates.

## Scope

- **BRRTRouter (or BFF support crate):** Proxy library (e.g. `brrtrouter::bff::proxy(req, route_meta, config)` or equivalent) that builds URL from config base URL for `route_meta.x_service` + `route_meta.downstream_path`, substitutes path/query from request, forwards HTTP, returns response.
- **Config:** Map from service key to base URL (host:port).
- **Askama:** For operations with `downstream_path` / `x_service`, generated handler is a thin call to the proxy library.
- **Integration:** End-to-end BFF from generated spec proxying to a backend microservice.

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 2.1 | Proxy library | [story-2.1-proxy-library.md](story-2.1-proxy-library.md) |
| 2.2 | Downstream base URL config | [story-2.2-downstream-base-url-config.md](story-2.2-downstream-base-url-config.md) |
| 2.3 | Askama proxy handler | [story-2.3-askama-proxy-handler.md](story-2.3-askama-proxy-handler.md) |
| 2.4 | BFF proxy integration | [story-2.4-bff-proxy-integration.md](story-2.4-bff-proxy-integration.md) |

## References

- `docs/BFF_PROXY_ANALYSIS.md` §2.2b, §2.2c, §5.3
- BRRTRouter: `src/server/service.rs`, `src/dispatcher/core.rs`, `src/spec/types.rs`
- RERP: `openapi/accounting/bff-suite-config.yaml`
