"""Run cargo (or custom) fmt when a workspace directory has changed vs HEAD.

Generic for any BRRTRouter consumer; configurable workspace_dir and fmt command.
"""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path


def _run(
    cmd: Sequence[str],
    cwd: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        capture_output=True,
        text=True,
    )


def run_workspace_fmt(
    project_root: Path,
    workspace_dir: str = "microservices",
    fmt_argv: Sequence[str] | None = None,
    extra_check_dirs: Sequence[str] | None = None,
) -> int:
    """If workspace_dir has changed vs HEAD, run fmt; if fmt changes files, exit 1.

    Returns 0 when no change needed or fmt ran with no new changes; 1 when
    fmt modified files (caller should add and recommit).
    """
    workspace_slash = workspace_dir.rstrip("/") + "/"
    # Any change under workspace_dir (vs HEAD) triggers fmt
    r = _run(
        ["git", "diff", "--name-only", "HEAD", "--", workspace_slash],
        cwd=project_root,
    )
    if r.returncode != 0:
        print("git diff failed; skipping workspace fmt", file=sys.stderr)
        return 0
    if not r.stdout.strip():
        return 0

    if fmt_argv is None:
        r = _run(
            ["cargo", "fmt"],
            cwd=project_root / workspace_dir,
        )
    else:
        r = _run(list(fmt_argv), cwd=project_root)
    if r.returncode != 0:
        err = r.stderr or ""
        print(f"fmt failed: {err}", file=sys.stderr)
        return 1

    check_dirs = list(extra_check_dirs) if extra_check_dirs else [workspace_slash]
    for d in check_dirs:
        r = _run(["git", "diff", "--exit-code", "--", d], cwd=project_root)
        if r.returncode != 0:
            dirs_str = " ".join(check_dirs)
            print(
                f"fmt changed {d}. Please run: git add {dirs_str} && git commit",
                file=sys.stderr,
            )
            return 1
    return 0
