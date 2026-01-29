"""Patch Cargo.toml path deps for BRRTRouter and lifeguard to git. Used in CI."""

import logging
import re
import subprocess
import sys
from pathlib import Path

from brrtrouter_tooling.helpers import find_cargo_tomls

log = logging.getLogger(__name__)

BRRTRouter_GIT = '{ git = "https://github.com/microscaler/BRRTRouter", branch = "main" }'
LIFEGUARD_GIT = '{ git = "https://github.com/microscaler/lifeguard", branch = "main" }'

PATH_DEP_BRRTRouter = re.compile(
    r'((?:brrtrouter|brrtrouter_macros)\s*=\s*\{\s*path\s*=\s*["\'][^"\']*BRRTRouter[^"\']*["\'][^}]*\})',
    re.MULTILINE,
)
PATH_DEP_LIFEGUARD = re.compile(
    r'((?:lifeguard|lifeguard-derive|lifeguard-migrate)\s*=\s*\{\s*path\s*=\s*["\'][^"\']*lifeguard[^"\']*["\'][^}]*\})',
    re.MULTILINE,
)
# Match path = "..." or path = '...' and optional trailing comma (for in-place replacement)
_PATH_ENTRY = re.compile(r'path\s*=\s*["\'][^"\']*["\'][\s,]*')


def _key_brrtrouter(full: str) -> str:
    return "brrtrouter_macros" if "brrtrouter_macros" in full.split("=")[0] else "brrtrouter"


def _key_lifeguard(full: str) -> str:
    s = full.split("=")[0]
    if "lifeguard-migrate" in s:
        return "lifeguard-migrate"
    if "lifeguard-derive" in s:
        return "lifeguard-derive"
    return "lifeguard"


def find_matches(text: str) -> list[tuple[str, str]]:
    """Return [(old_fragment, replacement)] for BRRTRouter and lifeguard path deps."""
    out: list[tuple[str, str]] = []
    for m in PATH_DEP_BRRTRouter.finditer(text):
        full = m.group(1)
        out.append((full, f"{_key_brrtrouter(full)} = {BRRTRouter_GIT}"))
    for m in PATH_DEP_LIFEGUARD.finditer(text):
        full = m.group(1)
        out.append((full, f"{_key_lifeguard(full)} = {LIFEGUARD_GIT}"))
    return out


def patch_file(
    p: Path, *, dry_run: bool = False, audit: bool = False
) -> tuple[bool, list[tuple[str, str]]]:
    """Patch one Cargo.toml: replace BRRTRouter/lifeguard path deps with git. Returns (changed, [(old, new)])."""
    if not p.exists():
        return False, []
    text = p.read_text().replace("\r\n", "\n").replace("\r", "\n")
    matches = find_matches(text)
    if not matches:
        return False, []

    if audit:
        return True, matches

    for old, new in matches:
        # Preserve extra keys (features, optional, package): replace only path = "..." with git/branch
        inner_m = re.search(r"\{\s*(.+)\s*\}\s*$", new)
        if inner_m:
            git_fragment = inner_m.group(1).strip() + ", "
            patched = _PATH_ENTRY.sub(git_fragment, old, count=1)
            patched = re.sub(r",\s*}", " }", patched)  # TOML inline table: no trailing comma
            if patched == old:
                patched = new
        else:
            patched = new
        text = text.replace(old, patched, 1)

    if "BRRTRouter" in text and re.search(r'path\s*=\s*["\'][^"\']*BRRTRouter', text):
        msg = f"{p} still contains path to BRRTRouter after patch"
        raise ValueError(msg)
    if re.search(r'path\s*=\s*["\'][^"\']*lifeguard', text):
        msg = f"{p} still contains path to lifeguard after patch"
        raise ValueError(msg)

    if not dry_run:
        p.write_text(text)
    return True, matches


def run_cargo_update(workspace_dir: Path) -> None:
    """Run cargo update -p for brrtrouter, brrtrouter_macros, lifeguard*."""
    for packages in (
        ["brrtrouter", "brrtrouter_macros"],
        ["lifeguard", "lifeguard-derive", "lifeguard-migrate"],
    ):
        try:
            cmd = ["cargo", "update", "--workspace"] + [x for p in packages for x in ("-p", p)]
            r = subprocess.run(cmd, cwd=workspace_dir, capture_output=True, text=True)
            if r.returncode != 0:
                combined = (r.stderr or "") + (r.stdout or "")
                if "did not match any packages" in combined:
                    continue
                if "no matching package named" in combined and re.search(r'_gen["\'`\s]', combined):
                    log.debug(
                        "Skipping cargo update (gen crates may not exist): %s", combined[:200]
                    )
                    continue
                msg = f"cargo update failed in {workspace_dir}: {r.stderr or r.stdout}"
                raise RuntimeError(msg)
        except FileNotFoundError as e:
            log.debug("cargo not in PATH: %s", e)
        except subprocess.CalledProcessError as e:
            stderr = (e.stderr or "") if hasattr(e, "stderr") else ""
            stdout = (e.stdout or "") if hasattr(e, "stdout") else ""
            combined = stderr + stdout
            if "no matching package named" in combined and re.search(r'_gen["\'`\s]', combined):
                log.debug("Skipping cargo update (gen crates may not exist): %s", combined[:200])
                continue
            msg = f"cargo update failed in {workspace_dir}: {combined}"
            raise RuntimeError(msg) from e


def run(
    root: Path,
    *,
    workspace_dir_name: str = "microservices",
    dry_run: bool = False,
    audit: bool = False,
) -> int:
    """Find Cargo.toml, patch BRRTRouter/lifeguard path deps, run cargo update in workspace_dir_name. Returns 0 on success, 1 on failure."""
    try:
        _run_impl(root, workspace_dir_name=workspace_dir_name, dry_run=dry_run, audit=audit)
        return 0
    except (ValueError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


def _run_impl(
    root: Path,
    *,
    workspace_dir_name: str = "microservices",
    dry_run: bool = False,
    audit: bool = False,
) -> None:
    cargo_tomls = find_cargo_tomls(root)
    patched_workspace = False
    any_changed = False

    for p in cargo_tomls:
        changed, matches = patch_file(p, dry_run=dry_run, audit=audit)
        if not changed:
            continue
        any_changed = True
        try:
            rel = p.relative_to(root)
        except ValueError:
            rel = p
        if workspace_dir_name in rel.parts:
            patched_workspace = True
        for old, new in matches:
            if audit:
                print(f"  {rel}: {old.strip()!r} -> {new!r}")
            elif dry_run:
                print(f"  {rel}: would replace {old.strip()!r} -> {new!r}")
            else:
                print(f"Patched {rel}")

    if audit:
        n_with = sum(1 for p in cargo_tomls if find_matches(p.read_text()))
        print(
            f"\nAudit: {len(cargo_tomls)} Cargo.toml scanned; {n_with} with BRRTRouter or lifeguard path deps."
        )
        return
    if dry_run:
        print("\nDry-run: would patch. Run without --dry-run to apply.")
        return
    if not any_changed:
        print("No Cargo.toml with BRRTRouter or lifeguard path deps found; nothing to patch.")
        return

    if patched_workspace:
        d = root / workspace_dir_name
        if (d / "Cargo.toml").exists():
            run_cargo_update(d)
            print(f"Ran cargo update in {workspace_dir_name}/")
