# URI / Request-Target — Epics and Stories Summary

**Source:** `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md`  
**Purpose:** Author Epic/Story docs and create/link GitHub issues.  
**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md) — every story requires
comprehensive **positive and negative** unit tests (see each story’s Unit tests section).  
**Build board:** [`BUILD_BOARD.md`](BUILD_BOARD.md)  
**Issues:** Epic 10 [#373](https://github.com/microscaler/BRRTRouter/issues/373), Epic 11 [#374](https://github.com/microscaler/BRRTRouter/issues/374)

---

## Epic 10 — Request-target parse & rebuild (100% URI compliance)

**Scope:** Inbound parsing (`parse_query_params`, path params), outbound rebuild
(`resolve_path_template` / proxy), error taxonomy, length limits, URI stack
unification, OpenAPI style fidelity, and automated proof of compliance.
**Out of scope:** RFC 10008 QUERY method (Epic 11).

| ID | Story | One-line description | Parent | Type | Labels |
|----|--------|----------------------|--------|------|--------|
| 10.1 | Spec matrix & golden corpus | Freeze the normative matrix (RFC 3986/9110/form-urlencoded) and ship a golden vector suite that defines Done. | Epic 10 | story | uri-request-target, story |
| 10.2 | Inbound query parse edge cases | Make `parse_query_params` correct for `+`, `%`, duplicates, empty keys/values, truncated escapes, and fragment boundaries. | Epic 10 | story | uri-request-target, story |
| 10.3 | Inbound path segment decode | Audit/fix path-param extraction so segment decode matches RFC 3986 and OpenAPI path templates. | Epic 10 | story | uri-request-target, story |
| 10.4 | Component-specific encoders | Separate path-segment vs query encoders; document `+` vs `%20` policy; keep conservative defaults. | Epic 10 | story | uri-request-target, story |
| 10.5 | Proxy path/query passthrough | When safe, forward original path-and-query octets instead of decode→map→encode. | Epic 10 | story | uri-request-target, story |
| 10.6 | Request-target length → 414 | Configurable max request-target octets; reject with 414 before dialling downstream. | Epic 10 | story | uri-request-target, story |
| 10.7 | Error taxonomy | Uri-build/composition errors ≠ upstream failures; stop mapping everything to 502. | Epic 10 | story | uri-request-target, story |
| 10.8 | Unify http URI stack | Eliminate `http` 0.2 vs 1.0 drift for request-target parsing/building (coordinate with may_minihttp). | Epic 10 | story | uri-request-target, story |
| 10.9 | OpenAPI style/explode fidelity | Round-trip `form`/`simple`/explode multi-value params through parse and proxy rebuild. | Epic 10 | story | uri-request-target, story |
| 10.10 | Property/fuzz compliance suite | Property tests: random Unicode + reserved → parse → rebuild → Uri OK → decode equals map. | Epic 10 | story | uri-request-target, story |
| 10.11 | may_minihttp request-line boundary | Document and test the exact bytes handed to `parse_request` (query/fragment stripping). | Epic 10 | story | uri-request-target, story |

---

## Epic 11 — HTTP QUERY method (RFC 10008)

**Scope:** First-class QUERY routing, CORS, OpenAPI binding, proxy/client support,
POST fallback guidance. **Does not** replace Epic 10 URI work.
**Testing:** Same [`TESTING_STANDARD.md`](TESTING_STANDARD.md) — positive + negative
unit tests required on every story (11.1–11.4).

| ID | Story | One-line description | Parent | Type | Labels |
|----|--------|----------------------|--------|------|--------|
| 11.1 | Method + router + CORS | Accept `QUERY`, route it, advertise in CORS allow-lists / preflight. | Epic 11 | story | uri-request-target, story, rfc10008 |
| 11.2 | OpenAPI QUERY operations | Spec/generator support for QUERY operations and request bodies. | Epic 11 | story | uri-request-target, story, rfc10008 |
| 11.3 | Proxy & client QUERY | Forward QUERY + body downstream; may_minihttp / http Method coverage. | Epic 11 | story | uri-request-target, story, rfc10008 |
| 11.4 | Accept-Query + POST fallback docs | Document `Accept-Query`, cache-key notes, and browser/edge POST fallback. | Epic 11 | story | uri-request-target, story, rfc10008 |

---

## Suggested delivery order (Epic 10)

```text
10.1 (matrix) ──┬──► 10.2 (inbound query)
                ├──► 10.3 (inbound path)
                └──► 10.11 (may_minihttp boundary)
         │
         ▼
10.4 (encoders) ──► 10.5 (passthrough) ──► 10.6 (414) ──► 10.7 (errors)
         │
         ▼
10.9 (OpenAPI style) ──► 10.10 (fuzz) ──► 10.8 (URI stack unify)
```

Epic 11 may start after 10.1; must not block 10.2–10.10.
