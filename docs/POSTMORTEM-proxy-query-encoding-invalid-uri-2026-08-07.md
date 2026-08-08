# Postmortem: BFF proxy `InvalidUri` / 502 when rebuilding downstream paths (2026-08-07)

- **Severity**: High for consumers that proxy query strings with spaces or
  non-ASCII (Loadlinker post-a-job provinces cascade).
- **Status**: Fixed in `resolve_path_template` (percent-encode path + query).
- **Consumer postmortem**: hauliage
  `docs/postmortems/postmortem-bff-provinces-space-502-2026-08-07.md`
- **Initial fix commit**: `854e3d9` — percent-encode path/query params  
  **Follow-up**: `59eda3b` — comprehensive positive/negative URI rebuild tests + this doc.

---

## What happened

Loadlinker’s BFF uses `brrtrouter::http::proxy_untyped` to forward
`GET /api/v1/locations/provinces?country=…` to the `locations` Kubernetes
Service. The browser sent a correctly encoded query
(`country=South%20Africa`). After the HTTP stack decoded query parameters into
`HandlerRequest::query_params`, `resolve_path_template` concatenated:

```text
/api/v1/locations/provinces?country=South Africa
```

`http::Uri` (via `http_legacy::Uri`) rejected the raw space:

```text
InvalidUri → ProxyError::InvalidPath("invalid uri character")
           → HandlerResponse::error(502, "invalid path: …")
```

Downstream was never dialed. Operators saw a generic **502** that looked like
a gateway/nginx failure.

---

## Root cause

`src/http/proxy.rs` `resolve_path_template`:

1. Substituted `{path}` placeholders with **raw** param values.
2. Appended query as `key=value` with **raw** decoded strings.
3. Passed the result to `Uri::parse` inside `proxy_untyped_inner`.

The inbound request was already valid HTTP. The bug was **re-serialization**
of decoded params without percent-encoding — a classic “decoded in, encode
out” miss.

### Failure modes (unencoded rebuild)

| Raw value class | Typical `Uri` outcome | Downstream impact |
|-----------------|----------------------|-------------------|
| Space / ASCII controls | **Parse error** → 502 `invalid path` | Incident class (South Africa) |
| `#` | Parses; remainder becomes **fragment** | Silent truncation of query |
| `&` / `=` in values | Parses; **extra query params** | Wrong/corrupted downstream query |
| Accents / CJK / emoji | Parser-dependent; unsafe on wire | Interop risk; must encode |

---

## Fix

Percent-encode **path segment values** and **query keys/values** with
`urlencoding::encode` when rebuilding the downstream path:

```rust
resolved_path.replace(&needle, urlencoding::encode(v).as_ref());
// …
qs.push_str(urlencoding::encode(k).as_ref());
qs.push('=');
qs.push_str(urlencoding::encode(v).as_ref());
```

Assumptions (documented in code):

- `HandlerRequest` path/query params are **decoded** once by the server.
- OpenAPI path params are **single segments** (`/` in a value becomes `%2F`).
- Encoding is applied once at rebuild time (do not pre-encode in handlers).

`proxy_untyped` still maps all `ProxyError` variants to HTTP 502; that mapping
is unchanged. The Uri rebuild no longer fails for legitimate geography names.

---

## Tests (`http::proxy::tests`)

Positive:

- ASCII-safe query (`country=ZA`) unchanged and parseable
- Spaces → `%20` (incident regression) + legacy unencoded must fail Uri
- Accents / diacritics (`Côte d'Ivoire`, `São Paulo`, `Québec`, …) encode and
  round-trip via `urlencoding::decode`
- Delimiters in values (`&`, `=`, `?`, `#`) stay inside one param
- `+`, `%`, CJK, emoji
- Empty query values + multi-param lists
- Path params with spaces / accents / embedded `/`
- Query **keys** with spaces
- Tabs / newlines in values

Negative / corruption guards:

- Unencoded space/controls → `Uri` parse **error**
- Unencoded `#` → fragment truncation (encode fixes)
- Unencoded `&`/`=` → extra params (encode fixes)
- `ProxyError::InvalidPath` display string stable for ops grep

Run:

```bash
cargo test --lib http::proxy::tests
```

---

## Corrective actions

| # | Action | Status |
|---|--------|--------|
| 1 | Percent-encode path + query in `resolve_path_template` | Done (`854e3d9`) |
| 2 | Unit coverage for spaces, accents, delimiters, unicode, negatives | Done (this change) |
| 3 | Consumer postmortems (BRRTRouter + Loadlinker app) | Done |
| 4 | Consumers bump `brrtrouter` git rev / rebuild BFF images | Loadlinker dogfood done via path patch; pin bump in hauliage follow-up |
| 5 | Distinct status/body for Uri-build vs upstream connect failures | Done (Story 10.7: composition → 400, DNS → 502) |

---

## Lessons

1. **Any code that rebuilds a URI from a param map must encode.** Passing
   through an already-encoded inbound request string is safer; rebuilding from
   decoded maps is not.
2. **Not every bad rebuild 502s** — `&` and `#` can “succeed” while corrupting
   semantics. Tests must cover corruption, not only `InvalidUri`.
3. **Geography and people names are hostile inputs** — spaces and accents are
   normal; ISO codes alone are an insufficient test matrix for BFF proxies.
4. **Map proxy construction errors loudly in docs** — the wire body
   `invalid path: invalid uri character` is the fingerprint for this class.

---

## Timeline (UTC, 2026-08-07)

| Time | Event |
|------|--------|
| ~07:40 | Isolated Loadlinker BFF 502 body `invalid path: invalid uri character` |
| ~07:45 | Root-caused to `resolve_path_template` missing percent-encoding |
| ~07:51 | Path-patched BFF image: `South%20Africa` → 401 (auth) instead of 502 |
| Later | `854e3d9` on `main`; expanded unit tests + this postmortem |
