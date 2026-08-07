# Audit: URI / request-target compliance & RFC 10008 (QUERY)

**Date:** 2026-08-07  
**Scope:** BRRTRouter inbound query parsing, BFF proxy URI rebuild, HTTP method
surface  
**Trigger:** Loadlinker `country=South Africa` → `InvalidUri` → 502  
**Status:** Living audit — encode fix landed; remaining gaps tracked below.

---

## 0. Two different problems (do not conflate)

| Concern | Spec | What it solves | What it does **not** solve |
|---------|------|----------------|----------------------------|
| **URI / request-target** encoding & parsing | RFC 3986, RFC 9110 §7 (request target), WHATWG URL / `application/x-www-form-urlencoded` | Safe rebuild of path + query after decode; spaces, accents, delimiters | Large structured search bodies |
| **HTTP QUERY method** | [RFC 10008](https://www.rfc-editor.org/info/rfc10008) (June 2026) | Safe + idempotent + (eventually) cacheable requests with a **body** | Percent-encoding of GET query strings |

Switching Loadlinker country form values to ISO codes reduces *one* hostile
input class. It does **not** replace URI rebuild correctness for provinces,
districts, free-text search, filter DSLs, or any other query that carries
spaces / Unicode / reserved characters.

RFC 10008 is the right tool when the **query itself** outgrows the URL
(complex filters, large boolean graphs). It is orthogonal to the 502 we just
fixed.

---

## 1. Normative stack for “URI parsing” in BRRTRouter

BRRTRouter is an HTTP/1.1 origin (and BFF proxy), not a browser. Relevant layers:

1. **RFC 9110 (HTTP Semantics)** — method, request target, message framing.
   Absolute-path or asterisk-form targets; recommends supporting targets of at
   least **8000 octets** (implementations vary).
2. **RFC 3986 (URI)** — `pchar`, query, percent-encoding; unreserved =
   `ALPHA / DIGIT / "-" / "." / "_" / "~"`.
3. **RFC 3986 §2.4** — decode for processing, **re-encode when reconstituting**
   a URI (the rule we violated).
4. **WHATWG URL / `application/x-www-form-urlencoded`** — how HTML and most
   browsers encode query *components* (`+` or `%20` for space). Inbound we use
   `url::form_urlencoded::parse`.
5. **OpenAPI 3.x parameter styles** — `form` / `simple` / explode; affects how
   arrays and objects appear in query/path (separate from percent-encoding).

IRIs (RFC 3987) are not first-class: we operate on UTF-8 strings and
percent-encode to ASCII request-targets (correct for HTTP/1.1 wire format).

---

## 2. Current BRRTRouter data path

```text
Inbound request-target
  → server::request::parse_query_params
       url::form_urlencoded::parse   // DECODE (+ and %xx → Unicode String)
  → HandlerRequest::{path_params, query_params}   // decoded
  → proxy::resolve_path_template
       urlencoding::encode           // RE-ENCODE (RFC 3986 unreserved set)
  → http 0.2 Uri::parse              // validate request-target shape
  → may_minihttp client request line
```

### Inbound (`parse_query_params`)

```118:129:src/server/request.rs
pub fn parse_query_params(path: &str) -> ParamVec {
    if let Some(pos) = path.find('?') {
        let query_str = &path[pos + 1..];
        url::form_urlencoded::parse(query_str.as_bytes())
            .map(|(k, v)| (Arc::from(k.as_ref()), v.to_string()))
            .collect()
```

- Splits on first `?` only (fragment after `#` is typically already stripped by
  the HTTP stack before path reaches us — verify at may_minihttp boundary).
- Form-urlencoded decode: `+` → space; `%XX` → byte; UTF-8 lossy via `url` crate
  behaviour.
- Preserves **duplicate keys** as multiple `ParamVec` entries (good).

### Outbound (`resolve_path_template`)

- `urlencoding::encode`: percent-encodes every byte except
  `ALPHA / DIGIT / "-" / "." / "_" / "~"` (RFC 3986 unreserved).
- Space → `%20` (not `+`).
- Same encoder for **path segments** and **query keys/values** (conservative;
  encodes `/` in path params as `%2F`).

### Asymmetry (acceptable, must stay intentional)

| Direction | Library | Space | `+` in input |
|-----------|---------|-------|----------------|
| Decode | `form_urlencoded` | from `+` or `%20` | becomes space |
| Encode | `urlencoding` | `%20` | `%2B` |

Round-trip changes *encoding form* (`+` → `%20`) but not *semantics* for
servers that accept both. Documented; do not “fix-encode” outbound with `+`
for request-targets that also feed non-form parsers.

---

## 3. Compliance scorecard (post encode-fix)

| Requirement | Spec | Status | Notes |
|-------------|------|--------|-------|
| Re-encode after decode when rebuilding URI | RFC 3986 §2.4 | **Fixed** | Was the South Africa 502 |
| Encode reserved chars in query values (`& = ? #`) | RFC 3986 | **Fixed** | Unit tests for corruption / truncation |
| Encode Unicode (accents, CJK) as UTF-8 percent-triples | RFC 3986 / 3629 | **Fixed** | `urlencoding` byte-wise UTF-8 |
| Path segment encoding (`/` not a delimiter) | RFC 3986 path | **Fixed** | `%2F` in path params |
| Validate rebuilt target with a URI parser | RFC 9110 request-target | **Partial** | `http` 0.2 `Uri` — not a full RFC 3986 validator; rejects some illegal forms |
| Preserve original query **byte string** when proxying | Gateway best practice | **Gap** | We rebuild from map; order of equal keys OK, but we cannot pass opaque query as-received |
| Request-target length limits / 414 | RFC 9110 | **Gap** | No explicit max; may_minihttp / peers may fail opaquely |
| Reject overlong / invalid `%` sequences inbound | RFC 3986 | **Inherited** | Via `form_urlencoded` — audit edge cases (`%`, `%GZ`, truncated) |
| OpenAPI style/explode arrays in proxy rebuild | OpenAPI 3 | **Gap** | Proxy forwards flat `ParamVec`; multi-value OK, style rewriting not modelled |
| Distinct error for Uri-build vs upstream failure | Ops | **Gap** | All `ProxyError` → HTTP 502 |
| HTTP **QUERY** method (RFC 10008) | RFC 10008 | **Not implemented** | Separate epic; see §5 |

---

## 4. Recommended hardening (URI layer — priority order)

1. **Passthrough option for proxy**  
   When `x-brrtrouter-downstream-path` has no path templates and the inbound
   path suffix should be forwarded, prefer forwarding the original
   path-and-query octets (or at least the original query string) instead of
   decode→map→encode. Eliminates an entire class of re-serialization bugs.

2. **Component-specific encoders**  
   - Path segment: encode per RFC 3986 `pchar` (or keep current conservative
     unreserved-only encoder).  
   - Query component: same unreserved-only is fine; optionally allow
     `sub-delims` unencoded where OpenAPI says so.  
   Document that we intentionally do **not** use `+` for space on the wire.

3. **Request-target budget**  
   Enforce a configurable max (default ≥ 8192 octets) → **414 URI Too Long**
   before dialling downstream.

4. **Inbound fuzz corpus**  
   Property tests: for random Unicode + reserved strings,
   `parse → resolve_path_template → Uri::parse` succeeds and
   `form_urlencoded` round-trip equals the decoded map.

5. **Error taxonomy**  
   Uri-build failures → **400** (client/gateway composition) or a dedicated
   problem+json; connect/upstream → **502/504**. Stop overloading 502.

6. **Upgrade URI type when may_minihttp allows**  
   Today proxy client uses `http` **0.2** (`http_legacy`) while the rest of
   the crate uses `http` **1.0**. One URI model reduces “passes one parser,
   fails another” drift.

---

## 5. RFC 10008 — The HTTP QUERY Method

### What it is

Published **June 2026** (Proposed Standard). Defines method **QUERY**:

- **Safe** and **idempotent** (like GET)
- Carries **request content** that defines the query (like POST)
- Responses may be **cacheable** with a cache key that includes the body
  (RFC 10008 §2.7) — browser/CDN cache support still catching up
- Servers advertise media types via **`Accept-Query`**
- **Not** a CORS-safelisted method → browser cross-origin requires preflight

It exists precisely because stuffing rich queries into the URI hits length and
encoding limits — the class of problem Loadlinker will eventually hit for
search/filter UX even with perfect percent-encoding.

### What it is not

- Not a replacement for RFC 3986 encoding of GET query strings  
- Not “URI parsing 2.0”  
- Not something OpenAPI/BRRTRouter can assume everywhere today

### Browser / ecosystem support (as of 2026-08)

| Surface | Support |
|---------|---------|
| `fetch(url, { method: "QUERY", body })` | Works in Chromium / Firefox / Safari **scripted** paths (method not forbidden); use **uppercase** `QUERY` (Fetch does not case-normalize it like GET) |
| HTML `<form method="QUERY">` | **No** — falls back to GET (body dropped); WHATWG HTML still open |
| Browser HTTP cache for QUERY | **Not implemented** in Chrome/Firefox measurements (identical QUERY hits origin twice) |
| CORS | Preflight required cross-origin |
| Intermediaries (CDN/WAF/envoy) | Uneven — may 405/400 or strip body |
| BRRTRouter / OpenAPI routing | **No first-class QUERY** yet (`http::Method` / router / CORS allow-lists need explicit work) |
| may_minihttp client | Method via `http` 0.2 `Method::from_bytes` — likely accepts extension methods; needs explicit test |

**Practical guidance:** keep GET + correct URI encoding for simple filters;
design rich search as POST *or* QUERY with a **POST fallback** until the edge
and OpenAPI story are complete. Do not block URI compliance work on QUERY
adoption.

---

## 6. Relation to the provinces incident

```text
Browser  GET ...?country=South%20Africa     // valid request-target
BRRTRouter decode  country = "South Africa"
proxy rebuild      ?country=South Africa    // illegal (pre-fix)
Uri::parse         InvalidUri
proxy_untyped      502 invalid path
```

Post-fix rebuild emits `country=South%20Africa` again. ISO codes would have
avoided *this* string but not `province=KwaZulu-Natal` (hyphen OK),
`province=Provence-Alpes-Côte d'Azur`, `q=foo&co`, etc.

---

## 7. Decision record

1. **Treat URI rebuild as a security/correctness surface** — fuzz + table-driven
   tests stay in `http::proxy::tests`; expand inbound `parse_query_params` tests
   the same way.
2. **Do not wait for RFC 10008** to finish GET/query compliance.
3. **Track QUERY as a separate epic**: method registration, CORS, OpenAPI
   `QUERY` operation binding, cache-key docs, POST fallback for browsers/edges.
4. **Product**: ISO codes welcome for machine keys; names remain valid API
   inputs and must survive the proxy.

**Execution:** Epic **10** (URI parse/rebuild) and Epic **11** (RFC 10008 QUERY)
live under [`docs/EPICS/URI_REQUEST_TARGET/`](EPICS/URI_REQUEST_TARGET/README.md).

---

## 8. References

- RFC 3986 — Uniform Resource Identifier (URI): Generic Syntax  
- RFC 9110 — HTTP Semantics  
- RFC 10008 — The HTTP QUERY Method  
- WHATWG URL Standard — `application/x-www-form-urlencoded`  
- Postmortems:  
  `docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md`  
  hauliage `docs/postmortems/postmortem-bff-provinces-space-502-2026-08-07.md`
