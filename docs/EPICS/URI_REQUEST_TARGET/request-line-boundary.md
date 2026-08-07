# Request-line → `raw_path` boundary (Story 10.11)

**Contract version:** 2026-08-07  
**Layers:** may_minihttp (`httparse`) → `Request::path()` → BRRTRouter `parse_request`

## What may_minihttp hands BRRTRouter

`may_minihttp::Request::path()` returns `httparse::Request::path`: the **request-target
token** from the request line (bytes between method and HTTP-version), as UTF-8 `&str`.
No further normalization is applied in may_minihttp before the call into
`brrtrouter::server::parse_request`.

| Form (RFC 9110 §7.1) | Example request-target | `Request::path()` |
|----------------------|------------------------|-------------------|
| origin-form | `/p?q=1` | `/p?q=1` (path **and** query) |
| absolute-form | `http://h/p?q=1` | full absolute URI string |
| asterisk-form | `*` | `*` |
| authority-form | `example.com:443` | as received (CONNECT; uncommon here) |

**Fragments:** httparse does **not** strip `#…`. If a client sends `#` in the
request-target, it appears in `path()` (illegal for HTTP request-targets; should
not reach us from conforming peers).

**Encoding:** Percent-encoded octets and `+` in the query are preserved verbatim
into `path()` (no decode at the front).

## Ownership matrix (Story 10.11 N1–N7)

| ID | Scenario | Owning layer | Behaviour (locked) |
|----|----------|--------------|--------------------|
| N1 | Raw space in request-target | **Front** (httparse) | Parse error (`Version`); connection fails before app |
| N2 | CTL (tab) in target | **Front** (httparse) | Parse error (`Token`) |
| N3 | Oversize target | **Gap** | httparse accepts ≥8k today; Story **10.6** adds 414 in BRRTRouter |
| N4 | NUL in target | **Front** (httparse) | Parse error (`Token`) |
| N5 | Fragment `#` | **Neither strips** | Passed through; app query parser may treat `#…` as value octets (see 10.2). Prefer peer/edge reject. |
| N6 | Malformed absolute-form | **Front** / app | httparse may still accept; app normalizes known `http(s)://` absolute-form to origin path+query |
| N7 | Ambiguous `//evil/p` | **Pass-through** | Accepted by httparse as origin-form; router likely 404 — documented, not rewritten |
| N8 | Front OK / app reject | **App** (10.7) | Composition/taxonomy after successful parse |

## BRRTRouter responsibilities

1. Call `request_target_for_app(req.path())` before routing / query parse
   (`src/server/request_target.rs`) so absolute-form becomes origin-form
   path+query.
2. Feed that string to `parse_query_params` (Story 10.2) and to the path used
   for radix match (query stripped at first `?`).
3. Do **not** treat `+` as space in path segments (Story 10.3).
4. Retain the post-normalization target string when Story **10.5** needs opaque
   query passthrough (no may_minihttp API change required — octets are already
   in `path()`).

## may_minihttp follow-ups (optional)

may_minihttp Issues are disabled on the fork; tracked in BRRTRouter
[#390](https://github.com/microscaler/BRRTRouter/issues/390) (not blockers for Pass):

- Optional strip of `#fragment` at decode time (defense in depth).
- Optional max request-target length before handing to the service (pairs with 10.6).

## Tests

- Unit: `src/server/request_target.rs`
- Contract: `tests/request_line_boundary_tests.rs` (httparse → app helpers)
