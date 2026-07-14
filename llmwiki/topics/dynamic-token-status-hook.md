# Dynamic JWT Token-Status Hook

**Status:** verified against the current P0 implementation (2026-07-14)

## Purpose

`JwksBearerProvider` owns cryptographic and standard-claim validation but cannot know an identity
provider's dynamic revocation or version state. Consumers can attach a `JwtTokenStatusChecker`
that returns `Active`, `Revoked`, `Stale`, `Unavailable`, or `Invalid` for already validated claims.

Any result other than `Active` rejects authentication. BRRTRouter keeps the public response uniform
(`401`, invalid bearer token); structured internal logging distinguishes denylist, version, and
status-dependency decisions without changing the response oracle.

## Ordering and cache behavior

- The checker runs after signature and standard-claim validation.
- It runs on both JWT claims-cache misses and hits; a claims-cache hit never bypasses revocation.
- It runs in `SecurityProvider::validate`, once per successful authorization attempt.
- `SecurityProvider::extract_claims` is a post-validation operation and does not repeat the dynamic
  lookup. Calling extraction alone does not establish authorization.
- Consumer implementations own authoritative lookup, timeout, and caching policy. BRRTRouter
  treats `Unavailable` as rejection and does not provide a fail-open mode.

This split lets Sesame avoid negative-caching active tokens while still performing one Redis
pipeline per successful protected request. See Sesame
[ADR-003](../../../seasame-idam/docs/ADR-003-token-status-dependency-outage.md).

## Public API

- `brrtrouter::security::JwtTokenStatus`
- `brrtrouter::security::JwtTokenStatusChecker`
- `JwksBearerProvider::token_status_checker`

## Code anchors

- `src/security/jwks_bearer/mod.rs`
- `src/security/jwks_bearer/validation.rs`
- `src/security/mod.rs`
- `tests/jwks_p0_hardening_tests.rs`

## Verification

`tests/jwks_p0_hardening_tests.rs` proves dynamic changes are rechecked after claims caching,
dependency unavailability fails closed, and validation plus claims extraction invokes the checker
once.

## Gaps / drift

- Full BRRTRouter pedantic lint has a large pre-existing backlog unrelated to this hook; the focused
  P0 test binary passes.
- Consumer-specific metrics and dependency SLOs belong to the consumer integration, not the generic
  provider hook.
