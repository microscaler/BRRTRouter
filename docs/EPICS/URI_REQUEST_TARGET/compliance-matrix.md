# Epic 10 — URI / request-target compliance matrix

**Done definition:** every row below is **Pass** under automated tests (no
“manual only” for core parse/rebuild). Later stories close Gap/Partial rows;
they do not redefine Done.

**Non-goals:** RFC 10008 HTTP QUERY (Epic 11); HTML form submission quirks
beyond WHATWG `application/x-www-form-urlencoded` decode.

**Harness:** `tests/uri_golden_harness.rs` + `tests/uri_golden/corpus.json`  
**Testing standard:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md)  
**Build board:** [`BUILD_BOARD.md`](BUILD_BOARD.md)

| Requirement ID | Requirement | Spec § | Component | Status | Test ID |
|----------------|-------------|--------|-----------|--------|---------|
| REQ-REENCODE | Re-encode after decode when rebuilding URI | RFC 3986 §2.4 | `uri_encode` + `resolve_path_template` | **Pass** | Story 10.4; `encode_query_positive_p1_*` |
| REQ-RESERVED-QUERY | Encode reserved in query values (`& = ? #`) | RFC 3986 §2 | `encode_query_component` | **Pass** | Story 10.4 N2–N4 |
| REQ-UNICODE | Encode Unicode (accents, CJK, emoji) as UTF-8 pct | RFC 3986 / 3629 | `uri_encode` | **Pass** | Story 10.4 P2; proxy accent tests |
| REQ-PATH-SEGMENT-ENC | Path segment encoding (`/` not delimiter) | RFC 3986 §3.3 | `encode_path_segment` | **Pass** | Story 10.4 P3/N5; slash path-param tests |
| REQ-COMPONENT-ENCODERS | Named path vs query encode APIs | RFC 3986 §2.3–2.4 | `http::uri_encode` | **Pass** | Story 10.4; no raw `urlencoding::encode` in proxy |
| REQ-URI-VALIDATE | Rebuilt target parses as URI | RFC 9110 §7 | `http` 0.2 `Uri` | **Partial** | P1–P10, N3–N6 (0.2 not full RFC 3986 validator) |
| REQ-INBOUND-FORM | Inbound query via form-urlencoded (`+` / `%20`) | WHATWG form-urlencoded | `parse_query_params` | **Pass** | P2, P3, P1, P6, P7, P10 |
| REQ-INBOUND-INVALID-PCT | Truncated / illegal `%` inbound | RFC 3986 | `parse_query_params` | **Pass** | Story 10.2: leave-as-is / lossy UTF-8; `parse_query_params_negative_n1`–`n4`; golden N1/N2 |
| REQ-INBOUND-PATH-DECODE | Path param pct-decode (`+` ≠ space) | RFC 3986 §3.3 | `decode_path_segment` + radix | **Pass** | Story 10.3; `decode_path_segment_*` / `path_decode_*` |
| REQ-PASSTHROUGH | Preserve original query octets when safe | Gateway practice | `resolve_downstream_target` | **Pass** | Story 10.5; `resolve_downstream_positive_p1_*` / `_negative_n3_*` |
| REQ-414 | Request-target length → 414 | RFC 9110 | parse + proxy | **Pass** | Story 10.6; `request_target_length_*` / `proxy_untyped_maps_*_414` |
| REQ-OPENAPI-STYLE | style/explode fidelity on rebuild | OpenAPI 3 | proxy / decode_param | **Gap** | Story 10.9 (P6 duplicates Pass as flat ParamVec) |
| REQ-ERROR-TAXONOMY | Uri-build ≠ upstream 502 | Ops | `proxy_untyped` | **Gap** | Story 10.7 |
| REQ-HTTP-STACK | Unify http 0.2 / 1.0 URI handling | Internal | proxy / server | **Gap** | Story 10.8 |
| REQ-FUZZ | Property/fuzz no-panic + Uri-OK | QA | parse + rebuild | **Gap** | Story 10.10 |
| REQ-BOUNDARY | may_minihttp request-line contract | RFC 9110 §7.1 | httparse + `request_target_for_app` | **Pass** | Story 10.11; `request-line-boundary.md`; `request_line_boundary_tests` |
| REQ-EMPTY-PATH | Missing `?` → empty params | — | `parse_query_params` | **Pass** | N8 |
| REQ-UNRESERVED | Unreserved `-._~` stable on encode | RFC 3986 §2.3 | encoder | **Pass** | P8 |
| REQ-CONTROLS | CTL in rebuild encoded / raw fails Uri | RFC 3986 | `resolve_path_template` | **Pass** | N6 |

## Story 10.1 golden IDs

| ID | Class | Covered by harness |
|----|-------|-------------------|
| P1–P10 | Positive | `uri_golden_positive_*` |
| N1–N8 | Negative | `uri_golden_negative_*` |

## Status legend

| Status | Meaning |
|--------|---------|
| **Pass** | Automated test asserts behaviour; CI green |
| **Partial** | Automated coverage exists; known limitation documented |
| **Inherited** | Behaviour from dependency; locked by golden (may tighten later) |
| **Gap** | Not Done; owned by a later story |

When a Gap closes, flip Status to **Pass** and keep the same Requirement ID.
