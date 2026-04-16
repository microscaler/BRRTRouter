"""Scan services for Tiltfile dynamic resolution."""

import argparse
import json
import sys
from pathlib import Path

import yaml

from brrtrouter_tooling.helpers import to_snake_case


def run(directory: str, base_port: int) -> dict | int:
    """Crawl directory for OpenAPI files and output a dictionary mapping."""
    search_path = Path(directory).resolve()
    if not search_path.exists():
        print(f"Error: Target directory {directory} does not exist.", file=sys.stderr)
        return 1

    services = []
    binary_names = {}
    ports = {}

    current_port = base_port

    seen_parents: set[Path] = set()
    for pattern in ("*/openapi.yaml", "*/openapi.yml"):
        for p in sorted(search_path.glob(pattern)):
            parent = p.parent
            if parent in seen_parents:
                continue
            seen_parents.add(parent)
            service_name = parent.name
            services.append(service_name)

            try:
                with p.open(encoding="utf-8") as f:
                    _ = yaml.safe_load(f)
                binary_names[service_name] = f"{to_snake_case(service_name)}_service_api"
            except Exception as e:  # noqa: BLE001
                print(f"Warning: Failed to parse {p}: {e}", file=sys.stderr)
                binary_names[service_name] = f"{to_snake_case(service_name)}_service_api"

            ports[service_name] = str(current_port)
            current_port += 1

    return {
        "services": services,
        "binary_names": binary_names,
        "ports": ports,
    }


def run_scan_argv() -> int:
    """Parse argv specifically for scan."""
    parser = argparse.ArgumentParser(prog="brrtrouter client tilt scan")
    parser.add_argument(
        "--dir", required=True, help="Directory to scan (e.g. microservices/openapi/trader)"
    )
    parser.add_argument(
        "--base-port", type=int, default=8002, help="Base port integer to increment from"
    )

    args = parser.parse_args(sys.argv[3:])
    output = run(args.dir, args.base_port)
    if isinstance(output, int):
        return output
    print(json.dumps(output, indent=2))
    return 0
