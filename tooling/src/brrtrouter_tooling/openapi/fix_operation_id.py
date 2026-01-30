"""Fix operationId casing in OpenAPI YAML: convert camelCase to snake_case.

Matches BRRTRouter linter rules (operation_id_casing): list_pets, get_user, create_asset, etc.
"""

from __future__ import annotations

import logging
import re
from pathlib import Path

from brrtrouter_tooling.helpers import find_openapi_files, is_snake_case, to_snake_case

logger = logging.getLogger(__name__)

# operationId line: groups (prefix, dq_value, sq_value, unquoted_value, trailing).
_OPID_RE = re.compile(
    r"^(\s*operationId:\s*)"
    r"(?:\"([^\"]*)\"|'([^']*)'|([A-Za-z0-9_\-]+))"
    r"(\s*(?:#.*)?)$"
)


def process_file(path: Path, dry_run: bool) -> tuple[int, list[tuple[int, str, str]]]:
    """Process one OpenAPI file. Returns (number of replacements, [(line_index, old_val, new_val), ...])."""
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=True)
    changes: list[tuple[int, str, str]] = []
    for i, line in enumerate(lines):
        stripped = line.rstrip("\r\n")
        m = _OPID_RE.match(stripped)
        if not m:
            continue
        raw = m.group(2) or m.group(3) or m.group(4)
        if raw is None or not raw:
            msg = f"Empty or missing operationId at {path}:{i + 1}"
            raise ValueError(msg)
        if is_snake_case(raw):
            continue
        new_val = to_snake_case(raw)
        if new_val == raw:
            logger.warning(
                "operationId %r at %s:%d is not snake_case and could not be normalized",
                raw,
                path,
                i + 1,
            )
            continue
        prefix, _, _, _, trailing = m.groups()
        if line.endswith("\r\n"):
            line_end = "\r\n"
        elif line.endswith("\n"):
            line_end = "\n"
        else:
            line_end = ""
        new_line = f"{prefix}{new_val}{trailing}{line_end}"
        changes.append((i, raw, new_val))
        lines[i] = new_line

    if not changes:
        return 0, []

    if not dry_run:
        path.write_text("".join(lines), encoding="utf-8")

    return len(changes), changes


def run(
    openapi_dir: Path,
    dry_run: bool = False,
    verbose: bool = False,
    rel_to: Path | None = None,
) -> tuple[int, int]:
    """
    Find openapi.yaml/openapi.yml under openapi_dir, fix operationId to snake_case.
    Returns (total_replacements, files_touched).
    """
    if not openapi_dir.is_dir():
        logger.warning(
            "openapi_dir is not a directory or does not exist: %s",
            openapi_dir,
        )
        return 0, 0
    base = rel_to or openapi_dir
    files = find_openapi_files(openapi_dir)
    total = 0
    touched = 0
    for p in files:
        n, changes = process_file(p, dry_run=dry_run)
        if n:
            total += n
            touched += 1
            if verbose:
                for i, old_v, new_v in changes:
                    print(f"  {p}:{i + 1}  {old_v!r} -> {new_v!r}")
            else:
                try:
                    rel = p.relative_to(base)
                except ValueError:
                    rel = p
                print(f"  {rel}: {n} operationId(s)")
    return total, touched
