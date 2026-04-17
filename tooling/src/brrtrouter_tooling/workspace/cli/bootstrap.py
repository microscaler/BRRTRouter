"""`hauliage bootstrap microservice` — Bootstrap a microservice from OpenAPI."""

import sys
from pathlib import Path

from brrtrouter_tooling.workspace.bootstrap.microservice import run_bootstrap_microservice


def run_bootstrap(args, project_root: Path) -> None:
    service_name = args.service_name
    port = getattr(args, "port", None)
    force_stubs = getattr(args, "force_stubs", False)
    rc = run_bootstrap_microservice(service_name, port, project_root, force_stubs=force_stubs)
    sys.exit(rc)
