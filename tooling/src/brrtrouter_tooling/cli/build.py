"""`brrtrouter build` — host-aware cargo/cross/zigbuild."""

import sys
from pathlib import Path

from brrtrouter_tooling.build.host_aware import run as run_host_aware


def run_build_argv(argv: list[str] | None = None) -> None:
    """Parse argv and run host-aware build (target, arch, --release, --workspace-dir)."""
    import argparse

    if argv is None:
        argv = sys.argv[2:] if len(sys.argv) > 2 else []  # skip 'brrtrouter build'
    ap = argparse.ArgumentParser(description="Host-aware build (cargo/cross/zigbuild)")
    ap.add_argument("target", help="workspace or <system>_<module>")
    ap.add_argument(
        "arch",
        nargs="?",
        default=None,
        help="amd64, arm64, arm7, or all (default: host arch)",
    )
    ap.add_argument("--release", action="store_true", help="Release build")
    ap.add_argument(
        "--workspace-dir",
        default="microservices",
        help="Workspace directory (default: microservices)",
    )
    ap.add_argument(
        "--project-root",
        type=Path,
        default=Path.cwd(),
        help="Project root (default: cwd)",
    )
    args = ap.parse_args(argv)
    extra = ["--release"] if args.release else []
    rc = run_host_aware(
        args.target,
        arch=args.arch,
        extra_args=extra if extra else None,
        project_root=args.project_root,
        workspace_dir=args.workspace_dir,
    )
    sys.exit(rc)
