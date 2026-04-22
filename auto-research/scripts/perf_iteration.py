#!/usr/bin/env python3
"""BRRTRouter auto-research: perf iteration checklist and optional local Rust gates.

Run from the repository root (the directory that contains the workspace Cargo.toml).
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def repo_root_from_script() -> Path:
    """Directory containing workspace Cargo.toml (brrtrouter package)."""
    return Path(__file__).resolve().parents[2]


def is_brrtrouter_root(root: Path) -> bool:
    cargo = root / "Cargo.toml"
    if not cargo.is_file():
        return False
    text = cargo.read_text(encoding="utf-8", errors="replace")
    return 'name = "brrtrouter"' in text


def print_checklist() -> None:
    print(
        """
BRRTRouter auto-research — one iteration (budget ≥ 30 minutes wall clock)
============================================================================

A — Sync & build (Tilt / release as per team)
   Example: tilt up / your CI image build / cargo build --release -p brrtrouter

B — Static analysis
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features

C — Correctness (full suite when possible)
   just nt
   # or: just test

D — Measurement (same host class as baseline)
   just bench-baseline-ms02      # when establishing / refreshing baseline
   just bench-against-ms02       # compare schema_validation_hot_path

Then: if gates pass and metrics improve → git commit on current branch (no PR).
Append a row to auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md

See: llmwiki/topics/auto-research-perf-loop.md
"""
    )


def run_local_gates(root: Path) -> int:
    """Run fmt + clippy + workspace tests (long-running)."""
    # Align with AGENTS.md / project standard (workspace-wide).
    steps = [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "clippy", "--workspace", "--all-targets", "--all-features"],
        ["cargo", "test", "--workspace"],
    ]
    for cmd in steps:
        print(f"+ {' '.join(cmd)}", flush=True)
        r = subprocess.run(cmd, cwd=root)
        if r.returncode != 0:
            return r.returncode
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--verify-root",
        action="store_true",
        help="Exit 0 only if this is the brrtrouter repo root.",
    )
    p.add_argument(
        "--run-local-gates",
        action="store_true",
        help="Run cargo fmt --check, workspace clippy, workspace cargo test.",
    )
    args = p.parse_args()
    root = repo_root_from_script()

    if not is_brrtrouter_root(root):
        print(
            f"error: expected brrtrouter workspace root; Cargo.toml not found or not brrtrouter: {root}",
            file=sys.stderr,
        )
        return 2

    if args.verify_root:
        return 0

    if args.run_local_gates:
        return run_local_gates(root)

    print_checklist()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
