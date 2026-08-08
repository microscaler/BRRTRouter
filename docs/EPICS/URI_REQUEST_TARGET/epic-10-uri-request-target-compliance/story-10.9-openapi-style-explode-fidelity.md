# Story 10.9 — OpenAPI style/explode fidelity

**GitHub issue:** [#383](https://github.com/microscaler/BRRTRouter/issues/383)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.2, 10.3, 10.4  
**Blocks:** 10.10  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

OpenAPI `style` / `explode` control how arrays and objects appear in path and
query. `decode_param_value` already understands some styles for handlers; proxy
rebuild must not flatten multi-value params incorrectly (e.g. lose duplicates or
mis-join arrays).

## Delivery

- Inventory OpenAPI styles used by BFF proxy routes (form/simple, explode true/false).
- Ensure `ParamVec` → query string rebuild emits valid multi-value forms:
  - explode: `id=1&id=2`
  - non-explode form: `id=1,2` (if we claim support)
- Path-style simple arrays covered or explicitly unsupported with clear error.
- Goldens for multi-value query through parse → proxy rebuild → parse.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | `form` + `explode=true` array | `id=1&id=2` (or documented) |
| P2 | `form` + `explode=false` array | `id=1,2` (if supported) |
| P3 | Duplicate keys round-trip | multiset equality after parse→rebuild→parse |
| P4 | Path `style=simple` scalar | correct segment |
| P5 | Scalar query form | `k=v` Uri-OK |
| P6 | Object form explode (if supported) | documented serialization |
| P7 | Default style when omitted | OpenAPI default |
| P8 | Spaces under form style | encoded; Uri-OK |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Unsupported style in spec | load/codegen/composition error; no silent wrong encode |
| N2 | deepObject if unsupported | fail closed; no panic |
| N3 | explode mismatch vs claimed support | only documented behaviour |
| N4 | Losing duplicates on rebuild | test fails if flattened incorrectly |
| N5 | Empty array/object | documented; no panic |
| N6 | Null optional param | omit vs empty per policy |
| N7 | Conflicting path vs query style | validation error |
| N8 | Reserved chars corrupt join | must encode components |

### Acceptance criteria (tests)

- [x] Table-driven style × explode for every **supported** cell.
- [x] Unsupported styles covered by N*.

## Acceptance criteria

- [x] Documented supported style/explode matrix for proxy rebuild.
- [x] Duplicate/multi-value query round-trips for supported styles.
- [x] Unsupported styles fail closed (composition error), not corrupt query.
- [x] Matrix row for OpenAPI style fidelity marked Pass.
- [x] Unit tests section complete (positive + negative).

## References

- [`openapi-style-explode-matrix.md`](../openapi-style-explode-matrix.md)
- `src/http/openapi_query.rs`
- `src/server/request.rs` `decode_param_value`
- OpenAPI 3.1 Parameter Object `style` / `explode`
- `src/http/proxy.rs` `resolve_path_template`
