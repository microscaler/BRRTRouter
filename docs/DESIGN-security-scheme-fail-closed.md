# Design: security schemes must fail closed

> **Status:** PROPOSED (2026-07-25)
> **Found by:** Sesame-IDAM authority audit (ADR-011), while checking whether
> `PlatformServiceAuth` was enforced or merely declared.

---

## 1. What is wrong today

When a generated service registers providers for the `securitySchemes` in its
OpenAPI document, the `apiKey` branch ends in this fallback:

```rust
let fallback = std::env::var("BRRTR_API_KEY").ok()
    .or_else(|| args.test_api_key.clone())
    .unwrap_or_else(|| "test123".to_string());
service.register_security_provider(&scheme_name, Arc::new(StaticApiKeyProvider { key: fallback, .. }));
```

So a scheme that is **declared in the spec but not configured** does not stop
the service, and does not disable the scheme. It installs a provider that
accepts a credential printed in the source of a public repository, and the
service reports itself healthy.

The operator sees a normal startup. The only signal is one line among hundreds:

```
[auth] register StaticApiKeyProvider scheme=ApiKeyHeader from=fallback key_len=7
```

`key_len=7` is `test123`. Nothing says "this scheme is now effectively open".

### Why it has not bitten yet

In Sesame the exposure is currently contained, for reasons that are all
accidents rather than defences:

- The scheme with the fallback (`ApiKeyHeader`) appears only in the
  **document-level** `security:` list, never on an individual operation.
- Empirically, presenting `X-API-KEY: test123` to an operation that relies on
  that document-level list is **rejected** — so the router does not appear to
  implement OpenAPI's top-level *OR* semantics, where any one listed scheme
  suffices.

That second point is load-bearing and undocumented. If someone made the router
spec-compliant on OR semantics — a reasonable-sounding correctness fix — then
**every operation inheriting the document-level list would immediately accept
`test123` instead of a JWT**. A correctness improvement in one place would
become an authentication bypass everywhere, with no test failing.

That is the real defect: the system's safety currently depends on an
undocumented deviation that a future contributor would be right to "fix".

---

## 2. Proposed changes

### 2.1 Never invent a credential

Remove the `"test123"` default. A declared scheme with no configuration is an
operator error, and the router should say so rather than paper over it.

Order of preference when a scheme cannot be configured:

1. **Refuse to start**, naming the scheme and the config key that would fix it.
   A service that cannot enforce its own contract should not accept traffic.
2. If startup must be preserved (a scheme used by no route, say), register a
   provider that **denies everything** and log at error level.

Never a working credential. A test fixture is not a default.

### 2.2 Make the test key explicit and loud

Test ergonomics are real, so keep a static key — but only when asked for:

- `--test-api-key <KEY>` or `BRRTR_API_KEY` **explicitly set** enables it.
- Absent both, there is no key.
- When one is used, log a single unmissable warning naming the scheme, and
  expose it on the health/readiness payload so a deployment check can assert
  "no insecure security providers" in a real environment.

### 2.3 Validate declared-versus-configured at startup

Enumerate every scheme referenced by any operation (including the
document-level list) and assert a real provider exists for each. Report all
problems at once rather than failing on the first, since configuration errors
cluster.

This is the check that would have surfaced the Sesame finding immediately
instead of during an unrelated audit.

### 2.4 Decide and document the AND/OR semantics, then test them

OpenAPI 3.1 §4.8.30: a `security` array is a list of alternatives — satisfying
**any one** entry authorises the request. Objects *within* one entry are ANDed.

The router's current behaviour does not match that, and the mismatch is
currently protective. Whatever is chosen, it must be explicit:

- If OR is implemented (spec-compliant), then a document-level list mixing a
  strong and a weak scheme becomes exactly as strong as its weakest member.
  The router should **warn at startup** when a `security` list mixes schemes of
  different strength, because that is almost never what an author means.
- If AND is kept (deviation), it must be documented prominently and tested,
  since it silently changes what every spec in the fleet means.

Either way: tests asserting the chosen semantics from both directions —
a request satisfying only the weak scheme, and one satisfying only the strong.

### 2.5 Report the effective posture

`GET /health` (or a dedicated endpoint) should be able to answer "which schemes
are enforced, by what kind of provider, and is any of them insecure". Security
posture that can only be determined by reading startup logs cannot be asserted
in CI.

---

## 3. Consequences

**Positive**

- A misconfigured deployment fails visibly at boot instead of running open.
- The AND/OR question stops being a landmine: whichever is chosen is tested, so
  changing it becomes a deliberate, failing-test decision.
- Deployments can assert "no insecure providers" as a gate.

**Negative**

- Breaking for anyone relying on the `test123` default, which is the point.
  Existing test setups must pass `--test-api-key` explicitly.
- Startup can now fail on configuration that previously "worked", which will
  surface latent misconfiguration in other services. That is a benefit
  arriving disguised as a cost.

---

## 4. Related

The consuming fix belongs in Sesame regardless of what the router does: the
document-level `security: [BearerAuth, ApiKeyHeader]` in six service specs
declares an alternative nobody intends. It should list only what each operation
actually accepts, so the specs stop depending on router behaviour to be safe.
