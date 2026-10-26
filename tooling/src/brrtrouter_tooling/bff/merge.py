"""Merge sub-service OpenAPI specs: paths, schemas, $ref updates, proxy extensions (Story 1.2).

Per-operation x-brrtrouter-downstream-path and x-service are set here when each path is merged;
no separate apply step is needed."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any

from brrtrouter_tooling.helpers import (
    downstream_path,
    gateway_public_path,
    load_yaml_spec,
    to_pascal_case,
)

logger = logging.getLogger(__name__)

_INSTANCE_KEYS = frozenset({"example", "examples", "enum", "default", "const"})
_NAME_MAPS = frozenset(
    {
        "properties",
        "patternProperties",
        "schemas",
        "definitions",
        "$defs",
        # OpenAPI maps keyed by name or status: `responses.default` is a response, not a value.
        "responses",
        "headers",
        "parameters",
        "requestBodies",
        "content",
        "paths",
        "callbacks",
    }
)


def _nullable_to_31(node: Any, names: bool = False) -> int:
    """Rewrite OpenAPI 3.0 `nullable` in place for a 3.1 document; return how many were rewritten.

    The BFF spec is 3.1 (JSON Schema 2020-12), where `nullable` is not a keyword: a sub-service spec
    still on 3.0 would carry `nullable: true` into it unread, and a null would then fail response
    validation. `type: T` becomes `type: [T, "null"]` (and `null` joins an `enum`); a schema without
    `type` (`$ref`, `allOf`, ...) becomes `anyOf: [<it>, {type: "null"}]`, keeping its description
    (`anyOf`, not `oneOf`: an untyped inner schema may accept null itself). Instance data (`example`,
    `examples`, `enum`, `default`, `const`) and `x-*` extensions are not walked.
    `nullable: false` is dropped. Only boolean `nullable` values are touched, so a property named
    `nullable` is left alone.
    """
    count = 0
    if isinstance(node, list):
        for item in node:
            count += _nullable_to_31(item)
        return count
    if not isinstance(node, dict):
        return 0
    if names:
        # A map of names to schemas (`properties`, `components.schemas`, ...): every key is a name,
        # even `default` or `enum`, and the map itself is not a schema.
        for value in node.values():
            count += _nullable_to_31(value)
        return count
    for key, value in list(node.items()):
        # Instance data and extensions are not schemas: a `{nullable: true}` there is a value.
        if key in _INSTANCE_KEYS or (isinstance(key, str) and key.startswith("x-")):
            continue
        count += _nullable_to_31(value, names=key in _NAME_MAPS)
    flag = node.get("nullable")
    if not isinstance(flag, bool):
        return count
    del node["nullable"]
    if not flag:
        return count
    t = node.get("type")
    if isinstance(t, str):
        node["type"] = [t, "null"]
        if isinstance(node.get("enum"), list) and None not in node["enum"]:
            node["enum"].append(None)
    elif isinstance(t, list):
        if "null" not in t:
            t.append("null")
    else:
        keep = {k: node.pop(k) for k in ("description", "title", "example", "default") if k in node}
        inner = dict(node)
        node.clear()
        node.update(keep)
        node["anyOf"] = [inner, {"type": "null"}]
    return count + 1


def _update_refs_in_value(val: Any, old_name: str, new_name: str) -> None:
    if isinstance(val, dict):
        if "$ref" in val and val["$ref"] == f"#/components/schemas/{old_name}":
            val["$ref"] = f"#/components/schemas/{new_name}"
        for v in val.values():
            _update_refs_in_value(v, old_name, new_name)
    elif isinstance(val, list):
        for it in val:
            _update_refs_in_value(it, old_name, new_name)


def _merge_schemas(
    all_schemas: dict[str, Any],
    service_name: str,
    schemas: dict[str, Any],
) -> None:
    for schema_name, schema_def in schemas.items():
        prefixed = f"{to_pascal_case(service_name)}{schema_name}"
        all_schemas[prefixed] = dict(schema_def) if isinstance(schema_def, dict) else schema_def
        if isinstance(schema_def, dict):
            _update_refs_in_value(all_schemas[prefixed], schema_name, prefixed)


# Standard OpenAPI verbs plus RFC 10008 QUERY (path-item `query:` / promoted
# `x-brrtrouter-query`). Must stay in sync with the merge loop in
# `_merge_one_sub_service`.
_HTTP_OPERATION_KEYS = frozenset(
    {
        "get",
        "post",
        "put",
        "patch",
        "delete",
        "options",
        "head",
        "trace",
        "query",
        "x-brrtrouter-query",
    }
)


def _update_refs_in_paths(paths: dict[str, Any], old_name: str, new_name: str) -> None:
    for path_def in paths.values():
        if not isinstance(path_def, dict):
            continue
        for method, op in path_def.items():
            if method not in _HTTP_OPERATION_KEYS or not isinstance(op, dict):
                continue
            if "requestBody" in op:
                _update_refs_in_value(op["requestBody"], old_name, new_name)
            if "responses" in op:
                for r in op["responses"].values():
                    if isinstance(r, dict) and "content" in r:
                        _update_refs_in_value(r["content"], old_name, new_name)


def _register_operation_id(
    operation_origin: dict[str, tuple[str, str, str]],
    operation_id: str | None,
    service: str,
    path: str,
    method: str,
) -> None:
    if not operation_id:
        return
    previous = operation_origin.get(operation_id)
    if previous is not None:
        previous_service, previous_path, previous_method = previous
        message = (
            f"Operation ID conflict: {operation_id!r} is defined by "
            f"service {previous_service!r} at "
            f"{previous_method.upper()} {previous_path!r} and by "
            f"service {service!r} at {method.upper()} {path!r}"
        )
        raise ValueError(message)
    operation_origin[operation_id] = (service, path, method)


def _component_refs(val: Any, kind: str) -> set[str]:
    """Names referenced as ``#/components/{kind}/<name>`` anywhere in ``val``."""
    prefix = f"#/components/{kind}/"
    found: set[str] = set()
    stack = [val]
    while stack:
        cur = stack.pop()
        if isinstance(cur, dict):
            ref = cur.get("$ref")
            if isinstance(ref, str) and ref.startswith(prefix):
                found.add(ref[len(prefix) :])
            stack.extend(cur.values())
        elif isinstance(cur, list):
            stack.extend(cur)
    return found


def _schema_closure(roots: set[str], schemas: dict[str, Any]) -> set[str]:
    """``roots`` plus every schema they reference, transitively (within one service spec)."""
    keep: set[str] = set()
    todo = [r for r in roots if r in schemas]
    while todo:
        name = todo.pop()
        if name in keep:
            continue
        keep.add(name)
        todo.extend(r for r in _component_refs(schemas[name], "schemas") if r in schemas)
    return keep


def _kept(oid: str, include_ops: set[str] | None, exclude_ops: set[str] | None) -> bool:
    if include_ops is not None:
        return oid in include_ops
    if exclude_ops is not None:
        return oid not in exclude_ops
    return True


def _select_operations(
    sname: str,
    spec: dict[str, Any],
    spec_path: Path,
    include_operations: set[str] | None,
    exclude_operations: set[str] | None,
) -> dict[str, dict[str, Any]]:
    """The operations a suite takes from a service, by spec path then method (spec order).

    Every operation unless filtered; a filter naming an operationId the spec lacks is an error.
    """
    if include_operations is not None and exclude_operations is not None:
        msg = f"Service {sname!r}: set include_operations or exclude_operations, not both"
        raise ValueError(msg)
    seen_ops: set[str] = set()
    kept: dict[str, dict[str, Any]] = {}
    for path, path_def in (spec.get("paths") or {}).items():
        if not isinstance(path_def, dict):
            continue
        for method, op in path_def.items():
            if method not in _HTTP_OPERATION_KEYS or not isinstance(op, dict):
                continue
            oid = op.get("operationId") or ""
            seen_ops.add(oid)
            if _kept(oid, include_operations, exclude_operations):
                kept.setdefault(path, {})[method] = op
    unknown = sorted(((include_operations or set()) | (exclude_operations or set())) - seen_ops)
    if unknown:
        msg = (
            f"Service {sname!r}: operation filter names operationIds not in {spec_path}: {unknown}"
        )
        raise ValueError(msg)
    return kept


def _merge_tags(
    spec: dict[str, Any], kept: dict[str, dict[str, Any]], filtered: bool, all_tags: set[str]
) -> None:
    if "tags" not in spec:
        return
    used = {t for ops in kept.values() for op in ops.values() for t in op.get("tags") or []}
    for t in spec["tags"]:
        name = t.get("name", str(t)) if isinstance(t, dict) else str(t)
        if not filtered or name in used:
            all_tags.add(name)


def _proxy_operation(
    op_def: dict[str, Any],
    sname: str,
    base_path: str,
    path: str,
    downstream_base_path: str | None,
    bff_impl_operations: set[str] | None,
) -> dict[str, Any]:
    op = dict(op_def)
    op["x-service"] = sname
    op["x-service-base-path"] = base_path
    # The service serves its paths under base_path unless the suite says
    # otherwise (downstream_base_path: "" for a version-free service).
    op["x-brrtrouter-downstream-path"] = downstream_path(
        base_path if downstream_base_path is None else downstream_base_path, path
    )
    # BFF gen owns proxy routes; only listed operations get impl overrides.
    op["x-brrtrouter-impl"] = op.get("operationId", "") in (bff_impl_operations or set())
    return op


def _merge_paths(
    sname: str,
    spec: dict[str, Any],
    kept: dict[str, dict[str, Any]],
    base_path: str,
    all_paths: dict[str, Any],
    operation_origin: dict[str, tuple[str, str, str]],
    gateway_path_style: str,
    bff_impl_operations: set[str] | None,
    downstream_base_path: str | None,
) -> None:
    """Merge the kept operations in spec order, as an unfiltered merge does; a path with no kept
    operation is left out together with its path-level properties."""
    for path, ops in kept.items():
        path_def = spec["paths"][path]
        gateway_path = gateway_public_path(base_path, path, gateway_path_style=gateway_path_style)
        existing = all_paths.get(gateway_path)
        if existing is None:
            existing = {}
        for key, value in path_def.items():
            if key not in _HTTP_OPERATION_KEYS:
                # Preserve path-level properties (parameters, summary, description, servers).
                if key not in existing:
                    existing[key] = value
                elif existing[key] != value:
                    logger.warning(
                        "Path-level property %r conflict for %s, keeping first", key, gateway_path
                    )
                continue
            if key not in ops:
                continue
            method = key
            op = _proxy_operation(
                ops[method], sname, base_path, path, downstream_base_path, bff_impl_operations
            )
            _register_operation_id(operation_origin, op.get("operationId"), sname, path, method)
            if method in existing:
                existing_service = (existing.get(method) or {}).get("x-service")
                if existing_service is not None and existing_service != sname:
                    msg = (
                        f"Path conflict: {gateway_path!r} method {method!r} already "
                        f"defined by service {existing_service!r}, duplicate from {sname!r}"
                    )
                    raise ValueError(msg)
            existing[method] = op
        all_paths[gateway_path] = existing


def _merge_components(
    sname: str,
    spec: dict[str, Any],
    kept: dict[str, dict[str, Any]],
    filtered: bool,
    all_schemas: dict[str, Any],
    all_paths: dict[str, Any],
    merged_params: dict[str, Any],
    param_origin: dict[str, str],
) -> None:
    """Schemas and parameters; a filtered service contributes only what its kept operations reach."""
    components = spec.get("components") or {}
    params = components.get("parameters") or {}
    kept_tree = {
        path: {k: v for k, v in spec["paths"][path].items() if k not in _HTTP_OPERATION_KEYS} | ops
        for path, ops in kept.items()
    }
    used_params = _component_refs(kept_tree, "parameters")

    if "schemas" in components:
        schemas = components["schemas"]
        if filtered:
            roots = _component_refs(kept_tree, "schemas")
            for pn in used_params:
                roots |= _component_refs(params.get(pn), "schemas")
            keep = _schema_closure(roots, schemas)
            schemas = {k: v for k, v in schemas.items() if k in keep}
        _merge_schemas(all_schemas, sname, schemas)
        for schema_name in schemas:
            prefixed = f"{to_pascal_case(sname)}{schema_name}"
            _update_refs_in_paths(all_paths, schema_name, prefixed)

    if "parameters" in components:
        if filtered:
            params = {k: v for k, v in params.items() if k in used_params}
        for pn, pd in params.items():
            if pn in merged_params and merged_params[pn] != pd:
                first_service = param_origin.get(pn, "?")
                msg = (
                    f"Parameter conflict for {pn!r}: already defined by service "
                    f"{first_service!r}, conflicting definition from {sname!r}"
                )
                raise ValueError(msg)
            merged_params.setdefault(pn, pd)
            param_origin.setdefault(pn, sname)


def _merge_one_sub_service(
    sname: str,
    base_path: str,
    spec_path: Path,
    all_schemas: dict[str, Any],
    all_tags: set[str],
    all_paths: dict[str, Any],
    merged_params: dict[str, Any],
    param_origin: dict[str, str],
    operation_origin: dict[str, tuple[str, str, str]],
    gateway_path_style: str = "as_spec",
    bff_impl_operations: set[str] | None = None,
    downstream_base_path: str | None = None,
    include_operations: set[str] | None = None,
    exclude_operations: set[str] | None = None,
) -> None:
    if not spec_path.exists():
        logger.warning("Spec file not found for service %r: %s", sname, spec_path)
        return
    spec = load_yaml_spec(spec_path)
    if str(spec.get("openapi", "")).startswith("3.0"):
        n = _nullable_to_31(spec)
        if n:
            logger.info("%s: %d OpenAPI 3.0 nullable schema(s) rewritten for the 3.1 BFF", sname, n)
    filtered = include_operations is not None or exclude_operations is not None
    kept = _select_operations(sname, spec, spec_path, include_operations, exclude_operations)
    _merge_tags(spec, kept, filtered, all_tags)
    _merge_paths(
        sname,
        spec,
        kept,
        base_path,
        all_paths,
        operation_origin,
        gateway_path_style,
        bff_impl_operations,
        downstream_base_path,
    )
    _merge_components(
        sname, spec, kept, filtered, all_schemas, all_paths, merged_params, param_origin
    )


def _schema_name_mapping(
    all_schemas: dict[str, Any],
    sub_services: dict[str, Any],
) -> dict[str, list[str]]:
    mapping: dict[str, list[str]] = {}
    for prefixed in sorted(all_schemas.keys()):
        prefixes = [to_pascal_case(sname) for sname in sub_services]
        matching = [p for p in prefixes if prefixed.startswith(p)]
        prefix = max(matching, key=len) if matching else None
        if prefix is not None:
            unprefixed = prefixed[len(prefix) :]
            if unprefixed not in mapping:
                mapping[unprefixed] = []
            mapping[unprefixed].append(prefixed)
    return mapping


def _service_prefix_for_schema(prefixed_key: str, sub_services: dict[str, Any]) -> str | None:
    """Return the longest matching service prefix (PascalCase) for this prefixed schema key.

    When service names produce overlapping prefixes (e.g. Account vs Accounting),
    the longest match is used so AccountingUser maps to Accounting, not Account.
    """
    matching = [
        to_pascal_case(sname)
        for sname in sub_services
        if prefixed_key.startswith(to_pascal_case(sname))
    ]
    return max(matching, key=len) if matching else None


def _update_all_refs_in_value(
    val: Any,
    mapping: dict[str, list[str]],
    all_schemas: dict[str, Any],
    service_prefix: str | None,
) -> None:
    """Update $ref from unprefixed to prefixed name.

    When service_prefix is set, resolve to the prefixed schema for that service only,
    so e.g. refs inside BillingUser resolve to BillingAddress, not AccountAddress.
    """
    if isinstance(val, dict):
        if "$ref" in val:
            ref = val["$ref"]
            if "#/components/schemas/" in ref:
                unprefixed = ref.split("#/components/schemas/")[-1]
                if unprefixed in mapping:
                    candidates = mapping[unprefixed]
                    if service_prefix is not None:
                        candidates = [p for p in candidates if p.startswith(service_prefix)]
                    for p in candidates:
                        if p in all_schemas:
                            val["$ref"] = f"#/components/schemas/{p}"
                            break
        for v in val.values():
            _update_all_refs_in_value(v, mapping, all_schemas, service_prefix)
    elif isinstance(val, list):
        for it in val:
            _update_all_refs_in_value(it, mapping, all_schemas, service_prefix)


def _add_error_schema(
    all_schemas: dict[str, Any],
    all_paths: dict[str, Any],
    sub_services: dict[str, Any],
) -> None:
    if "Error" not in all_schemas:
        all_schemas["Error"] = {
            "type": "object",
            "required": ["error", "message"],
            "properties": {
                "error": {"type": "string", "description": "Error code"},
                "message": {"type": "string", "description": "Human-readable error message"},
                "details": {
                    # OpenAPI 3.1 / JSON Schema 2020-12: `nullable` is not a keyword (a null would fail
                    # validation); a type union allows it.
                    "type": ["object", "null"],
                    "description": "Additional details",
                    "additionalProperties": True,
                },
            },
        }
    matches = []
    for sname in sorted(sub_services.keys()):
        pe = f"{to_pascal_case(sname)}Error"
        if pe in all_schemas:
            matches.append(pe)
    if len(matches) == 1:
        pe = matches[0]
        _update_refs_in_paths(all_paths, "Error", pe)
        all_schemas["Error"] = {"$ref": f"#/components/schemas/{pe}"}
    else:
        # Just use the generic Error schema we inserted above.
        _update_refs_in_paths(all_paths, "Error", "Error")


def merge_sub_service_specs(
    sub_services: dict[str, dict[str, Any]],
    info: dict[str, Any] | None = None,
    bff_impl_operations: set[str] | None = None,
) -> dict[str, Any]:
    """Merge sub-service specs into one BFF spec with prefixed schemas and proxy extensions.

    sub_services: name -> { base_path, spec_path (Path), include_operations? | exclude_operations? }
    info: optional openapi info override (title, version, description).
    """
    bff: dict[str, Any] = {
        "openapi": "3.1.0",
        "info": {
            "title": "BFF API",
            "description": "Backend for Frontend API (generated). Do not edit manually.",
            "version": "1.0.0",
        },
        "servers": [{"url": "/", "description": "BFF"}],
        "tags": [],
        "paths": {},
        "components": {"parameters": {}, "schemas": {}},
    }
    if info:
        bff["info"].update(info)

    all_schemas = bff["components"]["schemas"]
    all_tags: set[str] = set()
    all_paths: dict[str, Any] = {}
    merged_params = dict(bff["components"]["parameters"])
    param_origin: dict[str, str] = {}
    operation_origin: dict[str, tuple[str, str, str]] = {}

    from brrtrouter_tooling.bff.components import collect_security_schemes

    service_schemes = collect_security_schemes(sub_services)
    if service_schemes:
        bff["components"]["securitySchemes"] = service_schemes
    for sname, cfg in sorted(sub_services.items()):
        spec_path = cfg.get("spec_path")
        base_path = cfg.get("base_path", f"/api/{sname}")
        if isinstance(spec_path, str):
            spec_path = Path(spec_path)
        if not spec_path:
            logger.warning("No spec_path configured for service %r, skipping", sname)
            continue
        _merge_one_sub_service(
            sname,
            base_path,
            spec_path,
            all_schemas,
            all_tags,
            all_paths,
            merged_params,
            param_origin,
            operation_origin,
            gateway_path_style=cfg.get("gateway_path_style", "as_spec"),
            bff_impl_operations=bff_impl_operations,
            downstream_base_path=cfg.get("downstream_base_path"),
            include_operations=cfg.get("include_operations"),
            exclude_operations=cfg.get("exclude_operations"),
        )

    bff["components"]["parameters"] = merged_params
    mapping = _schema_name_mapping(all_schemas, sub_services)
    for pn, pd in merged_params.items():
        if isinstance(pd, dict):
            origin = param_origin.get(pn)
            prefix = to_pascal_case(origin) if origin else None
            _update_all_refs_in_value(pd, mapping, all_schemas, prefix)
    for prefixed_key, schema_def in all_schemas.items():
        if isinstance(schema_def, dict):
            prefix = _service_prefix_for_schema(prefixed_key, sub_services)
            _update_all_refs_in_value(schema_def, mapping, all_schemas, prefix)
    _add_error_schema(all_schemas, all_paths, sub_services)

    bff["tags"] = sorted([{"name": t} for t in all_tags], key=lambda x: x["name"])
    bff["paths"] = dict(sorted(all_paths.items()))
    bff["components"]["schemas"] = dict(sorted(all_schemas.items()))
    return bff
