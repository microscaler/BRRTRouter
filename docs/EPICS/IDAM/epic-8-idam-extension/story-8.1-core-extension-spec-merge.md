# Story 8.1 — Core + extension spec merge at build

**GitHub issue:** [#287](https://github.com/microscaler/BRRTRouter/issues/287)  
**Epic:** [Epic 8 — IDAM extension and build/deploy](README.md)

## Overview

Implement (or document) a build step that merges the reference IDAM core OpenAPI and the customer IDAM extension OpenAPI into one combined OpenAPI spec so BRRTRouter codegen produces a single IDAM service with both core and extension routes.

## Diagram: Build pipeline (merge → codegen)

```mermaid
flowchart LR
  Core["idam-core.openapi.yaml"]
  Ext["idam-extension.openapi.yaml"]
  Merge["Merge (distinct path prefixes)"]
  Combined["combined.openapi.yaml"]
  Codegen["BRRTRouter codegen"]
  Binary["Single IDAM binary"]

  Core --> Merge
  Ext --> Merge
  Merge --> Combined
  Combined --> Codegen
  Codegen --> Binary
```

## Diagram: Path namespace (no clashes)

```mermaid
flowchart TB
  subgraph Combined["Combined spec paths"]
    subgraph CorePaths["Core prefix"]
      C1["/api/identity/auth/*"]
      C2["/api/identity/user"]
      C3["/api/identity/factors/*"]
    end
    subgraph ExtPaths["Extension prefix"]
      E1["/api/identity/preferences"]
      E2["/api/identity/api-keys"]
      E3["/api/identity/api-keys/{key_id}"]
    end
  end
```

## Delivery

- **Merge logic:** Generator script or manual process: input = `idam-core.openapi.yaml` (reference) + `idam-extension.openapi.yaml` (customer); output = combined OpenAPI with no path clashes (core and extension use distinct path prefixes).
- **Codegen:** BRRTRouter codegen runs on the combined spec → one IDAM binary; core paths implemented by shared library or generated handlers; extension paths implemented by customer handlers.
- **Document:** How to run the merge (CLI or build target); how to add extension paths (schema and path conventions from Epic 6.2).

## Acceptance criteria

- [ ] Merge step produces a valid combined OpenAPI (no duplicate paths).
- [ ] BRRTRouter codegen on combined spec produces one IDAM service.
- [ ] Documented: how to add customer extension spec and run merge + codegen.

## References

- [IDAM Design: Core and Extension](../../../IDAM_DESIGN_CORE_AND_EXTENSION.md) §2.2 (Option I)
- [Epic 6.2 — Reference IDAM core OpenAPI](../epic-6-idam-contract/story-6.2-reference-idam-core-openapi.md)
