# Epic 3 — BFF ↔ IDAM auth/RBAC

**GitHub issue:** [#256](https://github.com/microscaler/BRRTRouter/issues/256)

## Overview

The BFF does not talk to Supabase directly for auth. It talks to **IDAM** for token validation and RBAC; IDAM talks to Supabase (GoTrue). This epic ensures the BFF OpenAPI has correct securitySchemes (e.g. JWKS from the same issuer IDAM uses), optional claims enrichment by calling IDAM, and RBAC available from JWT claims or an IDAM API.

## Scope

- **BFF spec:** securitySchemes and security so BRRTRouter validates JWT (e.g. JWKS bearer, issuer aligned with IDAM/Supabase).
- **Optional:** After JWT validation, call IDAM (e.g. “get user metadata/roles”) and merge into HandlerRequest (claims enrichment).
- **RBAC:** Document and implement path: roles/permissions from JWT (Auth Hooks) and/or from IDAM “get roles” API so BFF can authorize and enrich downstream.

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 3.1 | BFF OpenAPI securitySchemes | [story-3.1-bff-openapi-security-schemes.md](story-3.1-bff-openapi-security-schemes.md) |
| 3.2 | Optional claims enrichment | [story-3.2-optional-claims-enrichment.md](story-3.2-optional-claims-enrichment.md) |
| 3.3 | RBAC from JWT or IDAM API | [story-3.3-rbac-from-jwt-or-idam.md](story-3.3-rbac-from-jwt-or-idam.md) |

## References

- `docs/BFF_PROXY_ANALYSIS.md` §6
- BRRTRouter: `src/security/mod.rs`, `src/server/service.rs`
- IDAM: `idam/README.md`, `idam/common/src/supabase.rs`
