# ADR 003: Kubernetes downstream Service proxy for BFF

**Status:** Accepted  
**Date:** 2026-07-07  
**Context:** Hauliage PRD [`docs/PRD_k8s-native-bff-routing.md`](../../hauliage/docs/PRD_k8s-native-bff-routing.md) Phase 2

## Decision

1. Generated BFF proxy controllers delegate to **`brrtrouter::http::proxy_untyped`** instead of inline Askama template logic.
2. Downstream host = **`{x-service}.{POD_NAMESPACE}.svc.cluster.local`** when `POD_NAMESPACE` is set; otherwise short Service name (same-namespace cluster DNS).
3. Downstream port = **`HAULIAGE_SERVICE_HTTP_PORT`** env var, default **8080**. Per-service `{FLEET_PORT}` env vars are **removed**.
4. Each proxy request uses a **fresh `may_http::HttpClient` connection** (no `thread_local` cache) to prevent cross-service socket reuse.
5. Request/response bodies pass through as JSON `Value` where possible; non-JSON responses become `Value::String`. Full raw-byte / multipart pass-through requires a future dispatcher extension (FR-25).

## Consequences

- Regenerate hauliage `bff/gen/` after upgrading BRRTRouter.
- BFF Deployment should set `POD_NAMESPACE` (downward API) and `HAULIAGE_SERVICE_HTTP_PORT=8080`.
- Cross-namespace targets (sesame-idam) remain explicit URLs in security config, not the generic proxy.
