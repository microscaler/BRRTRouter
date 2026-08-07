# Story 10.9 — OpenAPI style/explode fidelity

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.2, 10.3, 10.4  
**Blocks:** 10.10

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

## Acceptance criteria

- [ ] Documented supported style/explode matrix for proxy rebuild.
- [ ] Duplicate/multi-value query round-trips for supported styles.
- [ ] Unsupported styles fail closed (composition error), not corrupt query.
- [ ] Matrix row for OpenAPI style fidelity marked Pass.

## References

- `src/server/request.rs` `decode_param_value`
- OpenAPI 3.1 Parameter Object `style` / `explode`
- `src/http/proxy.rs` `resolve_path_template`
