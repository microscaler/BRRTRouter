# Postmortem: JWKS verification robustness (sesame RFC-8037 casing incident)

- **Severity**: Low for BRRTRouter directly (it *tolerated* the defect — which
  is itself the problem); Medium as an observability/robustness gap that made
  the incident hard to diagnose.
- **Status**: Fixed. BRRTRouter now enforces exact RFC 7518/8037 casing
  (strict, case-sensitive), treats OKP `alg` as optional per RFC 8037, and
  rejects every non-conforming key with a precise diagnostic — no silent drops.
- **Decision**: we chose STRICT rejection over leniency. Tolerating wrong
  casing is what let the sesame bug hide; enforcing the RFC surfaces producer
  defects at the verifier, immediately.
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

## Fix — STRICT per RFC, and loud

Design decision: **do not tolerate non-RFC casing.** JOSE member values are
case-sensitive (RFC 7518 for `oct`/`RSA`/`EC` + alg codes; RFC 8037 for
`OKP`/`Ed25519`/`EdDSA`). Leniency is what let the sesame casing bug hide —
because BRRTRouter accepted the malformed key, the defect surfaced only in a
strict downstream consumer, far from its origin. Being strict makes a
producer's casing bug fail *here*, immediately and diagnosably.

`src/security/jwks_bearer/mod.rs` now:

- **Exact, case-sensitive matching** for every `kty` (`"oct"`, `"RSA"`,
  `"EC"`, `"OKP"`) and every `alg` — `==`, not `eq_ignore_ascii_case`. A key
  with `kty:"okp"` is rejected, not silently accepted.
- **RFC 8037 OKP rule**: `kty=="OKP"`, `crv=="Ed25519"` (REQUIRED, exact),
  `alg` OPTIONAL but exactly `"EdDSA"` when present.
- **Precise near-miss diagnostics**: a key whose `kty` matches an RFC type
  only case-insensitively is rejected with
  `"kty has non-RFC casing (…case-sensitive); key REJECTED — the producer
  must emit the exact RFC casing"`, naming `kid/kty/crv/alg/expected`. Every
  reject path (bad base64, missing components, decode failure, unrecognized
  kty) logs a specific `warn!`. No key is ever dropped silently.

## Corrective actions

| # | Action | Status |
|---|--------|--------|
| 1 | Strict, case-sensitive `kty`/`alg`/`crv` matching per RFC 7518/8037 | Done |
| 2 | `alg` optional on OKP per RFC 8037 (but exact when present) | Done |
| 3 | Reject wrong casing with a precise diagnostic; never drop a key silently | Done |
| 4 | Add a mock-JWKS round-trip test: OKP/Ed25519 key → sign → `decode` succeeds; plus negative cases (`okp`, `ED25519`, contradictory `alg`) assert REJECTION + the diagnostic. The existing OKP fixture in `tests/jwks_headers_integration_tests.rs` only tests cache headers, so it would NOT have caught this | TODO |
| 5 | Surface "JWKS refreshed, N accepted, M rejected" at info so drops are visible in normal ops | TODO |

## Lessons

- **Be strict in what you accept for security-critical, spec-fixed values.**
  The usual "be liberal in what you accept" (Postel) is wrong for JOSE key
  material: tolerating malformed input hides producer bugs and defers the
  failure to a stricter peer, far from the cause. Enforce the RFC.
- **Never drop input silently in a security path.** A rejected key must
  produce a specific diagnostic. The gap between "malformed JWKS key" and
  "generic 401" is where incidents hide.
- **Test the verifier against realistic JWKS, not just HMAC fixtures.** Every
  parser branch (OKP, EC, RSA) needs a sign→verify round-trip test, plus
  negative casing tests that assert rejection.
