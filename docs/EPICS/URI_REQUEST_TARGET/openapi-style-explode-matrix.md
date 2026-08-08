# OpenAPI style / explode — proxy rebuild support (Story 10.9)

**Authority for Done:** this matrix + unit tests `openapi_query_*` / golden P6.  
**Inbound decode** (handlers): `decode_param_value` — separate from this table.  
**Outbound proxy rebuild:** `encode_query_styled` / `resolve_path_template`.

## Supported (proxy rebuild)

| Location | style | explode | Wire form | API |
|----------|-------|---------|-----------|-----|
| query | `form` (default when omitted) | `true` (OpenAPI default) | `id=1&id=2` | `QueryRebuildStyle::FormExplode` |
| query | `form` | `false` | `id=1,2` (values encoded, then comma-joined) | `QueryRebuildStyle::FormNoExplode` |
| path | `simple` | n/a | single segment via `encode_path_segment` | path templates `{param}` |
| query | omitted | omitted | same as form + explode=true | default |

Notes:

- Spaces under form → `%20` (never `+`) via Story 10.4 encoders.
- Empty `ParamVec` → no `?` (Story 10.5 P6).
- Empty array (no pairs for a key) → key omitted.
- Optional / null → absent from `ParamVec` → omitted (no empty token invented).

## Unsupported (fail closed)

| style / mode | Behaviour |
|--------------|-----------|
| `deepObject` | `QueryStyleError::Unsupported` / composition error — **no** silent `a[b]=c` invent |
| `matrix`, `label` | Unsupported for **query** rebuild |
| `spaceDelimited`, `pipeDelimited` | Unsupported for **encode** (inbound decode still splits for handlers) |
| path array styles | Unsupported — path params are **scalars** only; unresolved/`{` → composition error |
| object form explode | Unsupported for proxy rebuild |

Calling `query_rebuild_style` with an unsupported name returns `Err` — callers must not fall back to a wrong encoding.

## Round-trip

`form` + `explode=true`: parse → `ParamVec` duplicates → rebuild → parse preserves multiset order.  
`form` + `explode=false`: rebuild emits one key; inbound form-urlencoded yields a single comma-joined value (handler `decode_param_value` splits for arrays).
