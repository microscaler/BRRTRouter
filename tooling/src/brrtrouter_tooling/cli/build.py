import sys
from pathlib import Path

from brrtrouter_tooling.build.host_aware import run as run_host_aware


def run_build_argv(argv: list[str] | None = None) -> None:
    import argparse

    if argv is None:
        argv = sys.argv[2:] if len(sys.argv) > 2 else []
    ap = argparse.ArgumentParser(description="Host-aware build (cargo/cross/zigbuild)")
    ap.add_argument("target", help="workspace or <system>_<module>")
    ap.add_argument(
        "arch", nargs="?", default=None, help="amd64, arm64, arm7, or all (default: host arch)"
    )
    ap.add_argument("--release", action="store_true", default=True, help="Release build (default)")
    ap.add_argument("--no-release", dest="no_release", action="store_true", help="Debug build")
    ap.add_argument(
        "--workspace-dir",
        default="microservices",
        help="Workspace directory (default: microservices)",
    )
    ap.add_argument(
        "--project-root", type=Path, default=Path.cwd(), help="Project root (default: cwd)"
    )
    ap.add_argument(
        "--package",
        help=(
            "Cargo package to pass to -p. Default from <system>_<module> is "
            "{module}_service_api_impl (brrtrouter-gen layout). "
            "Shorthand {module}_impl is expanded to that form. "
            "rerp_* names are passed through for legacy RERP workspaces."
        ),
    )
    args = ap.parse_args(argv)
    extra = [] if getattr(args, "no_release", False) else ["--release"]

    # We pass 'package' explicitly to run_host_aware
    rc = run_host_aware(
        args.target,
        arch=args.arch,
        extra_args=extra if extra else None,
        project_root=args.project_root,
        workspace_dir=args.workspace_dir,
        package=args.package,
    )
    sys.exit(rc)
