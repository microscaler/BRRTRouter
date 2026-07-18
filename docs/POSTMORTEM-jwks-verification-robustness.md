# Postmortem: JWKS verification robustness (sesame RFC-8037 casing incident)

- **Severity**: Low for BRRTRouter directly (it tolerated the defect);
  Medium as an observability/robustness gap that made the incident hard to
  diagnose and leaves a latent trap.
- **Status**: Fixed (OKP `alg` now optional per RFC 8037; skipped keys logged).
- **Origin**: sesame-idam published a JWKS with non-RFC-8037 casing
  (`"kty":"okp"`, `"crv":"ED25519"`). See the sesame postmortem for the
  origin defect. This document covers BRRTRouter's role and hardening.

## What happened

sesame's `identity-session-service` served JWKS keys with the wrong case for
`kty`/`crv`. A strict downstream verifier (opengroupware `og-auth`) rejected
every key and 401'd all tokens, prompting an investigation into whether
BRRTRouter — which fronts loadlinker/fleetingdns via `JwksBearerProvider` —
was also affected or needed a fix.

## Finding: BRRTRouter did NOT break on the casing

`src/security/jwks_bearer/mod.rs` parses JWKS by hand into
`jsonwebtoken::DecodingKey`s. The relevant facts (pre-fix):

- `kty` is matched **case-insensitively**: `kty.eq_ignore_ascii_case("OKP")`.
  So sesame's `"okp"` matched.
- `crv` was **never read** in the production parser — only `x` is consumed by
  `DecodingKey::from_ed_components(x)`. So `"ED25519"` vs `"Ed25519"` was
  irrelevant to BRRTRouter.

Therefore the casing bug, on its own, could not have skipped the key at the
BRRTRouter layer, and BRRTRouter needed **no correctness fix** for it.
loadlinker/fleetingdns delegate entirely to this provider and hardcode no
`kty`/`crv`/`kid`, so no consumer-side change was required either.

## But two real weaknesses were exposed

### 1. The OKP branch required the OPTIONAL `alg` field (latent trap)

The gate was:

```rust
if kty.eq_ignore_ascii_case("OKP") && alg.eq_ignore_ascii_case("EdDSA") { … }
```

RFC 8037 §2 makes `alg` **OPTIONAL** on an OKP JWK (`crv` is the required
discriminator). A spec-legal JWKS that omits `alg` — or cases it unexpectedly
— would fail this gate, the key would never enter the map, and every token
with that `kid` would 401. This is a live trap for any future producer that
emits a minimal, RFC-legal OKP key. It is adjacent to, but distinct from, the
sesame casing bug.

### 2. Skipped keys vanished silently (the debugging cliff)

A JWKS entry that matched no `(kty, alg)` branch fell off the end of the loop
with **no log line**. The only downstream signal was a generic
`MissingKey { kid }` → 401 at validation time, which reads as "key not found"
— giving an operator no hint that a key *was present in the JWKS but rejected
as malformed*. This is precisely why the sesame incident was hard to localize:
the symptom (401) was several layers removed from the cause (a dropped key).

## Fix

`src/security/jwks_bearer/mod.rs`:

- **`alg` optional (RFC 8037)**: accept an Ed25519 signing key identified by
  EITHER `crv≈"Ed25519"` OR `alg≈"EdDSA"`, provided any present `alg` does not
  contradict:
  ```rust
  let looks_ed25519 = kty.eq_ignore_ascii_case("OKP")
      && (crv.eq_ignore_ascii_case("Ed25519") || alg.eq_ignore_ascii_case("EdDSA"))
      && (alg.is_empty() || alg.eq_ignore_ascii_case("EdDSA"));
  ```
- **Loud skips**: `warn!` when an OKP key is missing `x` or fails to decode,
  and a catch-all `warn!` for any key that matches no branch — naming
  `kid`/`kty`/`crv`/`alg` so a rejected key is visible in logs instead of
  surfacing only as a generic 401.

## Corrective actions

| # | Action | Status |
|---|--------|--------|
| 1 | OKP branch: treat `alg` as optional (RFC 8037) | Done |
| 2 | Log skipped/rejected JWKS keys with kid/kty/crv/alg | Done |
| 3 | Add a mock-JWKS round-trip test: OKP/Ed25519 key → sign → `decode` succeeds; include lowercase `okp`/`ed25519` and no-`alg` variants (the existing OKP fixture in `tests/jwks_headers_integration_tests.rs` omits `alg` and only tests cache headers, so it would NOT have caught this) | TODO |
| 4 | Consider surfacing "JWKS refreshed, N keys accepted, M skipped" at info level so silent drops are visible in normal ops | TODO |

## Lessons

- **Be liberal in what you accept (RFC 8037), strict in what you emit.**
  BRRTRouter's case-insensitive `kty` was correct resilience; extend the same
  leniency to the optional `alg`.
- **Never drop input silently in a security path.** A rejected key must
  produce a diagnostic. The distance between "malformed JWKS key" and
  "generic 401" is where incidents hide.
- **Test the verifier against realistic JWKS, not just HMAC fixtures.** Every
  JWKS parser branch (OKP, EC, RSA) needs a sign→verify round-trip test.
