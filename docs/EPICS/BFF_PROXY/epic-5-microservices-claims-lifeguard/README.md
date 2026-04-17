# Epic 5 — Microservices: claims in handlers + Lifeguard row-based access

**GitHub issue:** [#258](https://github.com/microscaler/BRRTRouter/issues/258)

## Overview

Backend microservices must (1) receive and use claims (from BFF-forwarded headers or JWT), (2) pass claims into Lifeguard so Postgres RLS can enforce row-based access via `request.jwt.claims`, and (3) validate forwarded claims so only trusted BFF-enriched requests are honoured. This epic covers exposing jwt_claims to typed handlers, Lifeguard session claims API, and the microservice auth model.

## Scope

- **BRRTRouter:** TypedHandlerRequest or generated Request type exposes jwt_claims (or equivalent) so microservice handlers can read claims.
- **Lifeguard:** API to set session claims (e.g. `request.jwt.claims`) per request so RLS policies can use them.
- **Microservice auth:** Document and implement: validate forwarded claims (JWT or signed headers from BFF), bind claims to Lifeguard session for DB access.

## Stories

| Story | Title | Doc |
|-------|--------|-----|
| 5.1 | Expose jwt_claims to typed handlers | [story-5.1-expose-jwt-claims-typed-handlers.md](story-5.1-expose-jwt-claims-typed-handlers.md) |
| 5.2 | Lifeguard session claims | [story-5.2-lifeguard-session-claims.md](story-5.2-lifeguard-session-claims.md) |
| 5.3 | Microservice auth model | [story-5.3-microservice-auth-model.md](story-5.3-microservice-auth-model.md) |

## References

- `docs/BFF_PROXY_ANALYSIS.md` §7
- BRRTRouter: `src/typed/core.rs`, `src/dispatcher/core.rs`
- Lifeguard: `lifeguard/README.md`, `lifeguard/src/connection.rs`
