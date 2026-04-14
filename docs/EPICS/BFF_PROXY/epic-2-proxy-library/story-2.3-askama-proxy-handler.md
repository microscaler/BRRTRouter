# Story 2.3 — Askama proxy handler

**GitHub issue:** [#264](https://github.com/microscaler/BRRTRouter/issues/264)  
**Epic:** [Epic 2 — BFF proxy library](README.md)

## Overview

When RouteMeta has `downstream_path` and `x_service`, the generated handler should be a thin wrapper that calls the proxy library with the request, RouteMeta, and config—no per-route URL construction or HTTP logic in the template. Askama only passes through to the library.

## Delivery

- In the code generator (Askama templates): for operations whose RouteMeta has proxy metadata (e.g. `downstream_path` and `x_service` present), generate a handler that:
  - Receives the standard HandlerRequest (and route/RouteMeta).
  - Calls the proxy library function (e.g. `brrtrouter::bff::proxy(req, route_meta, config)`).
  - Returns the result (response) from the library.
- No conditional URL building or path concatenation in the template—only a single call to the proxy library.
- Non-proxy operations continue to generate existing handler stubs or custom logic as today.

## Acceptance criteria

- [ ] Generated code for a BFF operation with `x-brrtrouter-downstream-path` and `x-service` is a thin handler that calls the proxy library.
- [ ] No URL string building or path concatenation in the generated handler body.
- [ ] Generated handler compiles and runs when proxy library and config are available (validated in Story 2.4).
- [ ] Operations without proxy metadata still generate as before (no regression).

## Example

Generated handler should be conceptually: receive request and route; call `proxy(req, route_meta, config)`; return response. No in-template code for building URLs or issuing HTTP requests—only reference to the proxy library and RouteMeta/HandlerRequest types.

## Diagram

```mermaid
flowchart TB
  subgraph Spec["BFF spec"]
    Op[Operation with downstream_path, x_service]
  end
  subgraph Gen["Code generator (Askama)"]
    Check{Has proxy meta?}
    Thin[Generate thin handler: proxy(req, route_meta, config)]
    Other[Generate existing handler type]
  end
  subgraph Runtime["Runtime"]
    Lib[Proxy library]
  end
  Op --> Check
  Check -->|Yes| Thin --> Lib
  Check -->|No| Other
```

## References

- BRRTRouter: generator/templates (Askama), `src/spec/types.rs` (RouteMeta)
- `docs/BFF_PROXY_ANALYSIS.md` §5.2, §5.3
