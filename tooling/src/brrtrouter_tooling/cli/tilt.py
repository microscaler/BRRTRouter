"""`rerp tilt` subcommands: setup-kind-registry, setup-persistent-volumes, setup, teardown, logs."""

import sys
from pathlib import Path

from brrtrouter_tooling.tilt.logs import run as run_logs
from brrtrouter_tooling.tilt.scan import run_scan_argv
from brrtrouter_tooling.tilt.setup import run as run_setup
from brrtrouter_tooling.tilt.setup_kind_registry import run as run_setup_kind_registry
from brrtrouter_tooling.tilt.setup_persistent_volumes import (
    run as run_setup_persistent_volumes,
)
from brrtrouter_tooling.tilt.teardown import run as run_teardown


def run_tilt_argv() -> None:
    """Run tilt tool commands based on sys.argv."""
    if len(sys.argv) < 3:
        print("Usage: brrtrouter client tilt <command> [args...]", file=sys.stderr)
        print("Commands:", file=sys.stderr)
        print("  setup-kind-registry      - Setup local Docker registry for Kind", file=sys.stderr)
        print("  setup-persistent-volumes - Setup PersistentVolumes for RERP", file=sys.stderr)
        print("  setup                    - Setup Tilt environment", file=sys.stderr)
        print("  teardown                 - Teardown Tilt environment", file=sys.stderr)
        print("  scan                     - Scan OpenAPI dirs for Tilt mapping", file=sys.stderr)
        print("  logs <component>         - Tail Tilt logs for a component", file=sys.stderr)
        sys.exit(1)

    project_root = Path.cwd()
    t = sys.argv[2]

    if t == "setup-kind-registry":
        sys.exit(run_setup_kind_registry(project_root))
    if t == "setup-persistent-volumes":
        sys.exit(run_setup_persistent_volumes(project_root))
    if t == "setup":
        sys.exit(run_setup(project_root))
    if t == "teardown":
        remove_images = "--remove-images" in sys.argv
        remove_volumes = "--remove-volumes" in sys.argv
        system_prune = "--system-prune" in sys.argv
        sys.exit(
            run_teardown(
                project_root,
                remove_images=remove_images,
                remove_volumes=remove_volumes,
                system_prune=system_prune,
            )
        )
    if t == "scan":
        sys.exit(run_scan_argv())
    if t == "logs":
        if len(sys.argv) < 4:
            print("Usage: brrtrouter client tilt logs <component>", file=sys.stderr)
            sys.exit(1)
        component = sys.argv[3]
        sys.exit(run_logs(component, project_root))

    print(f"Error: Unknown tilt subcommand: {t}", file=sys.stderr)
    sys.exit(1)
