"""MCP tools for BRRTRouter: spec linting, code generation, BFF generation.

Each function is registered as an MCP tool and may be called by an AI assistant
to interact with the BRRTRouter toolchain.  All tools return a plain string
result (success message or error details) so the assistant can interpret them.
"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path
from typing import Any

import yaml

from brrtrouter_tooling.bff import generate_bff_spec
from brrtrouter_tooling.gen import (
    call_brrtrouter_generate,
    call_brrtrouter_generate_stubs,
)
from brrtrouter_tooling.helpers import is_snake_case, load_yaml_spec
from brrtrouter_tooling.openapi import fix_operation_id_run, validate_specs

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_HTTP_METHODS = {"get", "post", "put", "patch", "delete", "options", "head", "trace"}

# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _find_brrtrouter_gen(project_root: Path) -> Path | None:
    """Return path to a compiled brrtrouter-gen binary if available."""
    candidates = [
        project_root / "target" / "debug" / "brrtrouter-gen",
        project_root / "target" / "release" / "brrtrouter-gen",
    ]
    for c in candidates:
        if c.is_file():
            return c
    return None


def _run_brrtrouter_gen_lint(
    spec_path: Path,
    project_root: Path,
    brrtrouter_path: Path | None,
) -> str:
    """Run brrtrouter-gen lint and return combined stdout+stderr."""
    br_path = brrtrouter_path or project_root
    binary = _find_brrtrouter_gen(br_path)

    if binary:
        cmd = [str(binary), "lint", "--spec", str(spec_path), "--fail-on-error"]
    else:
        manifest = br_path / "Cargo.toml"
        if not manifest.exists():
            return "brrtrouter-gen binary not found and Cargo.toml not available for cargo run"
        cmd = [
            "cargo",
            "run",
            "--manifest-path",
            str(manifest),
            "--bin",
            "brrtrouter-gen",
            "--",
            "lint",
            "--spec",
            str(spec_path),
            "--fail-on-error",
        ]

    result = subprocess.run(
        cmd,
        check=False,
        capture_output=True,
        text=True,
        cwd=str(project_root),
    )
    output = (result.stdout + result.stderr).strip()
    return output or ("Lint passed (exit 0)" if result.returncode == 0 else "Lint failed (no output)")


def _collect_refs(obj: Any) -> list[str]:
    """Recursively collect all $ref values from a spec fragment."""
    refs: list[str] = []
    if isinstance(obj, dict):
        if "$ref" in obj:
            refs.append(obj["$ref"])
        for v in obj.values():
            refs.extend(_collect_refs(v))
    elif isinstance(obj, list):
        for item in obj:
            refs.extend(_collect_refs(item))
    return refs


def _check_schema_refs(
    paths: dict[str, Any],
    components: dict[str, Any],
    schemas: dict[str, Any],
    issues: list[str],
) -> None:
    """Append unresolved #/components/schemas/$ref entries to issues."""
    all_refs = _collect_refs(paths) + _collect_refs(components)
    for ref in all_refs:
        if ref.startswith("#/components/schemas/"):
            name = ref.removeprefix("#/components/schemas/")
            if name not in schemas:
                issues.append(f"Unresolved $ref: {ref!r}")


def _check_operation_ids(
    paths: dict[str, Any],
    issues: list[str],
) -> None:
    """Append issues for missing/non-snake_case/duplicate operationIds."""
    seen: set[str] = set()
    for path, path_item in paths.items():
        if not isinstance(path_item, dict):
            continue
        for method, operation in path_item.items():
            if method not in _HTTP_METHODS or not isinstance(operation, dict):
                continue
            op_id = operation.get("operationId")
            if not op_id:
                issues.append(f"Missing operationId for {method.upper()} {path}")
                continue
            if not is_snake_case(op_id):
                issues.append(
                    f"operationId {op_id!r} for {method.upper()} {path} is not snake_case"
                )
            if op_id in seen:
                issues.append(f"Duplicate operationId: {op_id!r}")
            seen.add(op_id)


def _check_number_formats(
    obj: Any,
    warnings: list[str],
    path_hint: str = "",
) -> None:
    """Append warnings for number fields that lack a format annotation."""
    if isinstance(obj, dict):
        if obj.get("type") == "number" and "format" not in obj:
            warnings.append(
                f"Number field at {path_hint!r} missing format: decimal or format: money"
            )
        for k, v in obj.items():
            _check_number_formats(v, warnings, f"{path_hint}.{k}")
    elif isinstance(obj, list):
        for i, item in enumerate(obj):
            _check_number_formats(item, warnings, f"{path_hint}[{i}]")


def _check_operations_conformance(
    paths: dict[str, Any],
    errors: list[str],
    warnings: list[str],
) -> bool:
    """Check operations for snake_case IDs, SSE usage, and error response format.

    Returns True if any 4xx/5xx responses are found.
    """
    seen_op_ids: set[str] = set()
    has_error_responses = False

    for path, path_item in (paths or {}).items():
        if not isinstance(path_item, dict):
            continue
        for method, operation in path_item.items():
            if method not in _HTTP_METHODS or not isinstance(operation, dict):
                continue
            op_id = operation.get("operationId")
            if not op_id:
                errors.append(f"Missing operationId: {method.upper()} {path}")
                continue
            if not is_snake_case(op_id):
                errors.append(f"operationId not snake_case: {op_id!r} ({method.upper()} {path})")
            if op_id in seen_op_ids:
                errors.append(f"Duplicate operationId: {op_id!r}")
            seen_op_ids.add(op_id)
            if operation.get("x-sse") and method != "get":
                errors.append(f"x-sse: true on non-GET operation: {op_id!r}")
            has_error_responses = _check_error_responses(
                operation, op_id, warnings, has_error_responses
            )

    return has_error_responses


def _check_error_responses(
    operation: dict[str, Any],
    op_id: str,
    warnings: list[str],
    has_error_responses: bool,
) -> bool:
    """Check a single operation's responses for RFC 7807 compliance.

    Returns True if any error responses are found (mutates warnings).
    """
    responses = operation.get("responses", {})
    for status_code, resp in responses.items():
        if not isinstance(resp, dict):
            continue
        code = str(status_code)
        if code.startswith(("4", "5")):
            has_error_responses = True
            content = resp.get("content", {})
            if content and "application/problem+json" not in content:
                warnings.append(
                    f"Error response {code} for {op_id!r} should use "
                    f"application/problem+json (RFC 7807)"
                )
    return has_error_responses


def _build_conformance_report(
    errors: list[str],
    warnings: list[str],
    suggestions: list[str],
) -> str:
    """Format errors, warnings, and suggestions into a readable report."""
    lines: list[str] = []
    if errors:
        lines.append(f"❌ Errors ({len(errors)}):")
        lines.extend(f"  - {e}" for e in errors)
    if warnings:
        lines.append(f"⚠️  Warnings ({len(warnings)}):")
        lines.extend(f"  - {w}" for w in warnings)
    if suggestions:
        lines.append(f"💡 Suggestions ({len(suggestions)}):")
        lines.extend(f"  - {s}" for s in suggestions)
    if not lines:
        return "✅ Spec is fully conformant with BRRTRouter requirements."
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Public tool functions
# ---------------------------------------------------------------------------


def lint_spec(spec_content: str, project_root: str | None = None) -> str:
    """Lint an OpenAPI spec string for BRRTRouter conformance.

    Performs two checks:
    1. Static Python checks: openapi version, operationId snake_case, missing $refs.
    2. brrtrouter-gen lint (if binary or Cargo.toml available at project_root).

    Args:
        spec_content: Full OpenAPI YAML content as a string.
        project_root: Optional path to a BRRTRouter workspace (for brrtrouter-gen lint).

    Returns:
        A string report of issues found, or a success message if none.
    """
    try:
        spec: dict[str, Any] = yaml.safe_load(spec_content)
    except yaml.YAMLError as e:
        return f"YAML parse error: {e}"

    if not isinstance(spec, dict):
        return "Error: spec root is not a YAML mapping"

    issues: list[str] = []

    if spec.get("openapi") != "3.1.0":
        issues.append(f"openapi version must be '3.1.0', got: {spec.get('openapi')!r}")

    info = spec.get("info") or {}
    if not info.get("title"):
        issues.append("info.title is missing")
    if not info.get("version"):
        issues.append("info.version is missing")

    paths = spec.get("paths") or {}
    if not paths:
        issues.append("No paths defined")
    else:
        _check_operation_ids(paths, issues)

    components = spec.get("components") or {}
    schemas = components.get("schemas") or {}
    _check_schema_refs(paths, components, schemas, issues)

    if issues:
        lines = ["Found issues:"] + [f"  - {i}" for i in issues]
        return "\n".join(lines)

    if project_root:
        pr = Path(project_root).resolve()
        with tempfile.NamedTemporaryFile(suffix=".yaml", mode="w", delete=False) as tf:
            tf.write(spec_content)
            tmp_path = Path(tf.name)
        try:
            lint_output = _run_brrtrouter_gen_lint(tmp_path, pr, None)
        finally:
            tmp_path.unlink(missing_ok=True)
        return f"Static checks passed.\nbrrtrouter-gen lint output:\n{lint_output}"

    return "All static checks passed. (Pass project_root to also run brrtrouter-gen lint.)"


def validate_openapi_dir(openapi_dir: str) -> str:
    """Validate all openapi.yaml files found under a directory.

    Loads each file and checks for YAML syntax errors and structural validity.

    Args:
        openapi_dir: Path to a directory to search for openapi.yaml files.

    Returns:
        A string listing any invalid specs, or a success message.
    """
    d = Path(openapi_dir).resolve()
    errors = validate_specs(d)
    if not errors:
        count = len(list(d.rglob("openapi.yaml"))) if d.exists() else 0
        return f"All {count} OpenAPI spec(s) under {d} are valid." if count else "No openapi.yaml files found."
    lines = [f"Found {len(errors)} invalid spec(s):"]
    for path, exc in errors:
        lines.append(f"  {path}: {exc}")
    return "\n".join(lines)


def fix_operation_ids(openapi_dir: str, dry_run: bool = True) -> str:
    """Convert all operationIds in a directory tree to snake_case.

    Args:
        openapi_dir: Root directory to scan for openapi.yaml files.
        dry_run: When True (default), report changes without writing files.

    Returns:
        Summary of operationIds that were (or would be) converted.
    """
    d = Path(openapi_dir).resolve()
    if not d.exists():
        return f"Directory not found: {d}"
    total, touched = fix_operation_id_run(d, dry_run=dry_run, verbose=True, rel_to=d)
    prefix = "[DRY-RUN] " if dry_run else ""
    if touched:
        return f"{prefix}Updated {touched} file(s), {total} operationId(s) converted to snake_case."
    return "No operationId casing changes needed."


def generate_project(
    spec_path: str,
    output_dir: str,
    project_root: str,
    brrtrouter_path: str | None = None,
    deps_config_path: str | None = None,
    package_name: str | None = None,
) -> str:
    """Generate a complete Rust project (gen crate) from an OpenAPI spec.

    Runs `brrtrouter-gen generate` and returns the command output.

    Args:
        spec_path: Path to the OpenAPI 3.1.0 YAML spec.
        output_dir: Directory where the generated crate will be written.
        project_root: Rust workspace root (cwd for cargo run).
        brrtrouter_path: Optional path to BRRTRouter checkout (defaults to ../BRRTRouter).
        deps_config_path: Optional path to brrtrouter-dependencies.toml.
        package_name: Optional Cargo package name for the generated crate.

    Returns:
        Command output (stdout + stderr) and exit status.
    """
    result = call_brrtrouter_generate(
        spec_path=Path(spec_path).resolve(),
        output_dir=Path(output_dir).resolve(),
        project_root=Path(project_root).resolve(),
        brrtrouter_path=Path(brrtrouter_path).resolve() if brrtrouter_path else None,
        deps_config_path=Path(deps_config_path).resolve() if deps_config_path else None,
        package_name=package_name,
        capture_output=True,
    )
    output = (result.stdout + result.stderr).strip()
    status = "succeeded" if result.returncode == 0 else f"failed (exit {result.returncode})"
    return f"generate {status}.\n{output}"


def generate_stubs(
    spec_path: str,
    impl_dir: str,
    component_name: str,
    project_root: str,
    brrtrouter_path: str | None = None,
    force: bool = False,
    sync: bool = False,
) -> str:
    """Generate implementation stub files (impl crate) from an OpenAPI spec.

    Runs `brrtrouter-gen generate-stubs`.  Existing files with the
    `BRRTROUTER_USER_OWNED` sentinel are not overwritten unless `--force` is used.

    Args:
        spec_path: Path to the OpenAPI 3.1.0 YAML spec.
        impl_dir: Directory where impl stubs will be written.
        component_name: Name of the gen crate (used for import paths in stubs).
        project_root: Rust workspace root (cwd for cargo run).
        brrtrouter_path: Optional path to BRRTRouter checkout.
        force: When True, overwrite existing stub files.
        sync: When True, only patch stub signatures (preserves user body).

    Returns:
        Command output and exit status.
    """
    result = call_brrtrouter_generate_stubs(
        spec_path=Path(spec_path).resolve(),
        impl_dir=Path(impl_dir).resolve(),
        component_name=component_name,
        project_root=Path(project_root).resolve(),
        brrtrouter_path=Path(brrtrouter_path).resolve() if brrtrouter_path else None,
        force=force,
        sync=sync,
        capture_output=True,
    )
    output = (result.stdout + result.stderr).strip()
    status = "succeeded" if result.returncode == 0 else f"failed (exit {result.returncode})"
    return f"generate-stubs {status}.\n{output}"


def generate_bff(
    suite_config_path: str,
    output_path: str | None = None,
    base_dir: str | None = None,
    validate: bool = True,
) -> str:
    """Generate a merged BFF OpenAPI spec from a suite config YAML.

    Merges multiple downstream service specs into a single BFF spec with
    path prefixing, schema prefixing, and proxy routing extensions.

    Args:
        suite_config_path: Path to bff-suite-config.yaml.
        output_path: Optional override for the output spec path.
        base_dir: Optional base directory for resolving relative paths in the config.
        validate: When True, validate the generated spec after writing.

    Returns:
        Path to the generated spec, or an error message.
    """
    try:
        out = generate_bff_spec(
            suite_config_path=Path(suite_config_path).resolve(),
            output_path=Path(output_path).resolve() if output_path else None,
            base_dir=Path(base_dir).resolve() if base_dir else None,
            validate=validate,
        )
    except (ValueError, OSError, FileNotFoundError) as e:
        return f"BFF generation failed: {e}"
    return f"BFF spec generated: {out}"


def check_spec_conformance(spec_content: str) -> str:
    """Check an OpenAPI spec for BRRTRouter-specific conformance rules.

    Performs a comprehensive review of the spec:
    - Version must be 3.1.0
    - All operationIds must be snake_case and unique
    - Error responses should use application/problem+json
    - ProblemDetails schema should be present when error responses are defined
    - $ref targets must exist in components
    - Number fields should have format: decimal or format: money
    - SSE endpoints (x-sse: true) should be GET operations

    Args:
        spec_content: Full OpenAPI YAML content as a string.

    Returns:
        A formatted conformance report with warnings, errors, and suggestions.
    """
    try:
        spec: dict[str, Any] = yaml.safe_load(spec_content)
    except yaml.YAMLError as e:
        return f"YAML parse error: {e}"

    if not isinstance(spec, dict):
        return "Error: spec root is not a YAML mapping"

    errors: list[str] = []
    warnings: list[str] = []
    suggestions: list[str] = []

    if spec.get("openapi") != "3.1.0":
        errors.append(f"openapi must be '3.1.0', got {spec.get('openapi')!r}")

    info = spec.get("info") or {}
    if not info.get("title"):
        errors.append("info.title is missing")
    if not info.get("version"):
        errors.append("info.version is missing")

    components = spec.get("components") or {}
    schemas = components.get("schemas") or {}
    paths = spec.get("paths") or {}

    has_error_responses = _check_operations_conformance(paths, errors, warnings)

    if has_error_responses and "ProblemDetails" not in schemas:
        suggestions.append(
            "Add a ProblemDetails schema to components/schemas for RFC 7807 error responses"
        )

    _check_number_formats(schemas, warnings, "components.schemas")
    _check_schema_refs(paths, components, schemas, errors)

    return _build_conformance_report(errors, warnings, suggestions)


def inspect_generated_dir(gen_dir: str) -> str:
    """Inspect a generated (gen crate) directory and summarise its contents.

    Reads the generated handlers, controllers, and config to give a quick
    overview of what was produced by brrtrouter-gen.

    Args:
        gen_dir: Path to the generated crate directory.

    Returns:
        Summary of generated files and handler/controller names.
    """
    d = Path(gen_dir).resolve()
    if not d.exists():
        return f"Directory not found: {d}"

    handlers_dir = d / "src" / "handlers"
    controllers_dir = d / "src" / "controllers"
    config_file = d / "config" / "config.yaml"

    lines: list[str] = [f"Generated crate: {d}"]

    if handlers_dir.exists():
        handler_files = sorted(
            p.stem for p in handlers_dir.glob("*.rs") if p.stem not in {"mod", "types"}
        )
        lines.append(f"\nHandlers ({len(handler_files)}):")
        lines.extend(f"  - {h}" for h in handler_files)
    else:
        lines.append("\nNo handlers/ directory found")

    if controllers_dir.exists():
        controller_files = sorted(p.stem for p in controllers_dir.glob("*.rs") if p.stem != "mod")
        lines.append(f"\nControllers ({len(controller_files)}):")
        lines.extend(f"  - {c}" for c in controller_files)
    else:
        lines.append("\nNo controllers/ directory found")

    if config_file.exists():
        lines.append(f"\nConfig: {config_file} (present)")

    cargo_toml = d / "Cargo.toml"
    if cargo_toml.exists():
        content = cargo_toml.read_text()
        for line in content.splitlines():
            if line.startswith("name ="):
                lines.append(f"\nPackage: {line.strip()}")
                break

    return "\n".join(lines)


def inspect_impl_dir(impl_dir: str) -> str:
    """Inspect an implementation (impl crate) directory and summarise its contents.

    Lists handler files and indicates which ones have the BRRTROUTER_USER_OWNED
    sentinel (i.e. user-customised and protected from regeneration).

    Args:
        impl_dir: Path to the impl crate directory.

    Returns:
        Summary of impl files, which are user-owned vs generated stubs.
    """
    d = Path(impl_dir).resolve()
    if not d.exists():
        return f"Directory not found: {d}"

    handlers_dir = d / "src" / "handlers"
    lines: list[str] = [f"Impl crate: {d}"]

    if not handlers_dir.exists():
        return f"{d}: no src/handlers/ directory found"

    user_owned: list[str] = []
    stubs: list[str] = []

    for rs_file in sorted(handlers_dir.glob("*.rs")):
        if rs_file.stem == "mod":
            continue
        content = rs_file.read_text()
        if "BRRTROUTER_USER_OWNED" in content:
            user_owned.append(rs_file.stem)
        else:
            stubs.append(rs_file.stem)

    if user_owned:
        lines.append(f"\nUser-owned handlers ({len(user_owned)}):")
        lines.extend(f"  \U0001f512 {h}" for h in user_owned)
    if stubs:
        lines.append(f"\nGenerated stubs ({len(stubs)}):")
        lines.extend(f"  \U0001f4c4 {s}" for s in stubs)

    return "\n".join(lines)


def list_spec_operations(spec_path: str) -> str:
    """List all operations defined in an OpenAPI spec.

    Args:
        spec_path: Path to an OpenAPI YAML spec file.

    Returns:
        Formatted list of METHOD /path operationId for each operation.
    """
    p = Path(spec_path).resolve()
    if not p.exists():
        return f"File not found: {p}"
    try:
        spec = load_yaml_spec(p)
    except Exception as e:  # noqa: BLE001
        return f"Failed to load spec ({type(e).__name__}): {e}"

    paths = spec.get("paths", {})
    lines: list[str] = []
    for path, path_item in sorted(paths.items()):
        if not isinstance(path_item, dict):
            continue
        for method in _HTTP_METHODS:
            operation = path_item.get(method)
            if not isinstance(operation, dict):
                continue
            op_id = operation.get("operationId", "(no operationId)")
            summary = operation.get("summary", "")
            sse = " [SSE]" if operation.get("x-sse") else ""
            lines.append(f"  {method.upper():7} {path:<40} {op_id}{sse}  {summary}")

    if not lines:
        return "No operations found in spec."
    return f"Operations in {p.name}:\n" + "\n".join(lines)

