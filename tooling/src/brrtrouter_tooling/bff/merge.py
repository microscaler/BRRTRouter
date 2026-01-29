"""Merge sub-service OpenAPI specs: paths, schemas, $ref updates, proxy extensions (Story 1.2).

Per-operation x-brrtrouter-downstream-path and x-service are set here when each path is merged;
no separate apply step is needed."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

from brrtrouter_tooling.bff._text import _to_pascal_case


def _load_spec(p: Path) -> dict[str, Any]:
    with p.open() as f:
        return yaml.safe_load(f)


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
        prefixed = f"{_to_pascal_case(service_name)}{schema_name}"
        all_schemas[prefixed] = dict(schema_def) if isinstance(schema_def, dict) else schema_def
        if isinstance(schema_def, dict):
            _update_refs_in_value(all_schemas[prefixed], schema_name, prefixed)


def _update_refs_in_paths(paths: dict[str, Any], old_name: str, new_name: str) -> None:
    methods = {"get", "post", "put", "patch", "delete", "options", "head", "trace"}
    for path_def in paths.values():
        if not isinstance(path_def, dict):
            continue
        for method, op in path_def.items():
            if method not in methods or not isinstance(op, dict):
                continue
            if "requestBody" in op:
                _update_refs_in_value(op["requestBody"], old_name, new_name)
            if "responses" in op:
                for r in op["responses"].values():
                    if isinstance(r, dict) and "content" in r:
                        _update_refs_in_value(r["content"], old_name, new_name)


def _downstream_path(base_path: str, bff_path: str) -> str:
    """Exact path on downstream: base_path + path (normalized)."""
    base = base_path.rstrip("/")
    path = bff_path.strip("/")
    return f"{base}/{path}" if path else base


def _merge_one_sub_service(
    sname: str,
    base_path: str,
    spec_path: Path,
    all_schemas: dict[str, Any],
    all_tags: set[str],
    all_paths: dict[str, Any],
    merged_params: dict[str, Any],
) -> None:
    if not spec_path.exists():
        return
    spec = _load_spec(spec_path)

    if "tags" in spec:
        for t in spec["tags"]:
            all_tags.add(t.get("name", str(t)) if isinstance(t, dict) else str(t))

    if "paths" in spec:
        for path, path_def in spec["paths"].items():
            if not isinstance(path_def, dict):
                continue
            path_def = dict(path_def)
            for method in list(path_def.keys()):
                if method not in (
                    "get",
                    "post",
                    "put",
                    "patch",
                    "delete",
                    "options",
                    "head",
                    "trace",
                ):
                    continue
                op = path_def[method]
                if not isinstance(op, dict):
                    continue
                op = dict(op)
                op["x-service"] = sname
                op["x-service-base-path"] = base_path
                op["x-brrtrouter-downstream-path"] = _downstream_path(base_path, path)
                path_def[method] = op
            all_paths[path] = path_def

    if "components" in spec and "schemas" in spec["components"]:
        _merge_schemas(all_schemas, sname, spec["components"]["schemas"])
        for schema_name in spec["components"]["schemas"]:
            prefixed = f"{_to_pascal_case(sname)}{schema_name}"
            _update_refs_in_paths(all_paths, schema_name, prefixed)

    if "components" in spec and "parameters" in spec.get("components", {}):
        for pn, pd in spec["components"]["parameters"].items():
            if pn not in merged_params:
                merged_params[pn] = pd


def _schema_name_mapping(
    all_schemas: dict[str, Any],
    sub_services: dict[str, Any],
) -> dict[str, list[str]]:
    mapping: dict[str, list[str]] = {}
    for prefixed in sorted(all_schemas.keys()):
        for sname in sorted(sub_services.keys()):
            prefix = _to_pascal_case(sname)
            if prefixed.startswith(prefix):
                unprefixed = prefixed[len(prefix) :]
                if unprefixed not in mapping:
                    mapping[unprefixed] = []
                mapping[unprefixed].append(prefixed)
    return mapping


def _service_prefix_for_schema(prefixed_key: str, sub_services: dict[str, Any]) -> str | None:
    """Return the service prefix (PascalCase) for this prefixed schema key, or None if none match."""
    for sname in sub_services:
        prefix = _to_pascal_case(sname)
        if prefixed_key.startswith(prefix):
            return prefix
    return None


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
                            val["$ref"] = ref.replace(unprefixed, p)
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
                    "type": "object",
                    "nullable": True,
                    "description": "Additional details",
                    "additionalProperties": True,
                },
            },
        }
    for sname in sorted(sub_services.keys()):
        pe = f"{_to_pascal_case(sname)}Error"
        if pe in all_schemas:
            _update_refs_in_paths(all_paths, "Error", pe)
            all_schemas["Error"] = {"$ref": f"#/components/schemas/{pe}"}
            break


def merge_sub_service_specs(
    sub_services: dict[str, dict[str, Any]],
    info: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Merge sub-service specs into one BFF spec with prefixed schemas and proxy extensions.

    sub_services: name -> { base_path, spec_path (Path) }
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

    for sname, cfg in sorted(sub_services.items()):
        spec_path = cfg.get("spec_path")
        base_path = cfg.get("base_path", f"/api/{sname}")
        if isinstance(spec_path, str):
            spec_path = Path(spec_path)
        if spec_path:
            _merge_one_sub_service(
                sname,
                base_path,
                spec_path,
                all_schemas,
                all_tags,
                all_paths,
                merged_params,
            )

    bff["components"]["parameters"] = merged_params
    mapping = _schema_name_mapping(all_schemas, sub_services)
    for prefixed_key, schema_def in all_schemas.items():
        if isinstance(schema_def, dict):
            prefix = _service_prefix_for_schema(prefixed_key, sub_services)
            _update_all_refs_in_value(schema_def, mapping, all_schemas, prefix)
    _add_error_schema(all_schemas, all_paths, sub_services)

    bff["tags"] = sorted([{"name": t} for t in all_tags], key=lambda x: x["name"])
    bff["paths"] = dict(sorted(all_paths.items()))
    bff["components"]["schemas"] = dict(sorted(all_schemas.items()))
    return bff
