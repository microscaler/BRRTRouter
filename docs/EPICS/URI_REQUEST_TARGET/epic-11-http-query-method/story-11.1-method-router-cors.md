# Story 11.1 — Method + router + CORS

**GitHub issue:** _(create)_  
**Epic:** [Epic 11 — HTTP QUERY](README.md)

## Overview

Accept `QUERY` as a first-class method in routing and CORS preflight responses.

## Delivery

- Ensure method parsing accepts `QUERY` (http 1.x / legacy bridge).
- Router matches OpenAPI/routes registered for QUERY.
- CORS: include `QUERY` in `Access-Control-Allow-Methods` when enabled; preflight succeeds.
- Reject unknown methods with 405 as today.

## Acceptance criteria

- [ ] Same-origin QUERY reaches a registered handler.
- [ ] CORS preflight lists QUERY when CORS is on.
- [ ] Tests for allow and 405 paths.

## References

- RFC 10008 §2, §4 (Security / CORS)
- `src/server/cors_setup.rs`, `src/router/`
