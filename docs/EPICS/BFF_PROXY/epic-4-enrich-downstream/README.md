# Epic 4 — Enrich downstream with claims/RBAC

**GitHub issue:** [#257](https://github.com/microscaler/BRRTRouter/issues/257)

## Overview

When the BFF proxies to a backend microservice, it must enrich the downstream request with claim-derived headers (e.g. X-User-Id, X-Roles, X-Permissions) so the backend can enforce RBAC and row-based access without re-validating the JWT. This epic adds claim header injection to the proxy library and optional configurable claim→header mapping.

## Scope

- **Proxy library:** When building the downstream request, add headers derived from jwt_claims and/or enriched_claims (e.g. X-User-Id, X-Roles).
- **Config or OpenAPI extension:** Define which claims map to which header names so enrichment is configurable rather than hard-coded per handler.

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 4.1 | Proxy claim headers | [story-4.1-proxy-claim-headers.md](story-4.1-proxy-claim-headers.md) |
| 4.2 | Configurable claim→header mapping | [story-4.2-configurable-claim-header-mapping.md](story-4.2-configurable-claim-header-mapping.md) |

## References

- `docs/BFF_PROXY_ANALYSIS.md` §5.5
- Epic 2 (proxy library), Epic 3 (claims/RBAC)
