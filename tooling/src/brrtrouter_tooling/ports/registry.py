"""Port registry: assignments, update_config_files (RERP-style layout)."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any

import yaml

from brrtrouter_tooling.discovery import (
    bff_service_to_suite,
    bff_suite_config_path,
    service_to_suite,
)
from brrtrouter_tooling.ports.layout import resolve_layout

START_PORT = 8001
RESERVED_PORTS = [8080]
TILT_MANAGED_RANGE = (8001, 8099)


def _update_openapi_servers(
    service_name: str,
    port: int,
    project_root: Path,
    layout: dict[str, str],
) -> None:
    """Update OpenAPI server URLs for Swagger 'Try it' to use the correct localhost port."""
    openapi_dir = project_root / layout["openapi_dir"]
    suite = bff_service_to_suite(project_root, service_name, layout)
    if suite is not None:
        path = bff_suite_config_path(project_root, suite, layout)
        if not path.exists():
            return
        with path.open() as f:
            data = yaml.safe_load(f) or {}
        meta = data.setdefault("metadata", {})
        servers = meta.get("servers") or []
        localhost_url = f"http://localhost:{port}"
        found = False
        for s in servers:
            if isinstance(s, dict) and "localhost" in str(s.get("url", "")):
                s["url"] = localhost_url
                found = True
                break
        if not found:
            servers.append({"url": localhost_url, "description": "Local development server (BFF)"})
        meta["servers"] = servers
        with path.open("w") as f:
            yaml.dump(data, f, default_flow_style=False, sort_keys=False)
        print(f"Updated {path} (BFF localhost server -> {localhost_url})")
        return

    suite = service_to_suite(project_root, service_name, layout)
    if not suite:
        return
    spec_path = openapi_dir / suite / service_name / "openapi.yaml"
    if not spec_path.exists():
        return
    with spec_path.open() as f:
        data = yaml.safe_load(f) or {}
    servers = data.get("servers") or []
    localhost_url = f"http://localhost:{port}/api/v1/{suite}/{service_name}"
    desc = "Local development (direct to service, port from port-registry)"
    new_list = []
    replaced = False
    for s in servers:
        if isinstance(s, dict) and "localhost" in str(s.get("url", "")):
            if not replaced:
                new_list.append({"url": localhost_url, "description": desc})
                replaced = True
        else:
            new_list.append(s)
    if not replaced:
        new_list.insert(0, {"url": localhost_url, "description": desc})
    data["servers"] = new_list
    with spec_path.open("w") as f:
        yaml.dump(data, f, default_flow_style=False, sort_keys=False)
    print(f"Updated {spec_path} (localhost server -> {localhost_url})")


class PortRegistry:
    """Manages port assignments (RERP-style layout)."""

    def __init__(
        self,
        registry_file: Path,
        project_root: Path,
        layout: dict[str, Any] | None = None,
    ):
        self.registry_file = registry_file
        self.project_root = project_root
        self._layout = resolve_layout(layout)
        self.registry = self._load_registry()

    def _load_registry(self) -> dict:
        if not self.registry_file.exists():
            return {
                "version": "1.0",
                "next_port": START_PORT,
                "reserved_ports": RESERVED_PORTS,
                "assignments": {},
                "metadata": {
                    "description": "Port registry for microservices",
                    "last_updated": None,
                    "notes": "Ports start at 8001. Port 8080 is reserved due to conflicts.",
                },
            }
        with self.registry_file.open() as f:
            import json

            return json.load(f)

    def _save_registry(self) -> None:
        import json

        self.registry["metadata"]["last_updated"] = datetime.now().isoformat()
        with self.registry_file.open("w") as f:
            json.dump(self.registry, f, indent=2)

    def _find_next_available_port(self) -> int:
        assigned = set(self.registry["assignments"].values())
        reserved = set(self.registry.get("reserved_ports", RESERVED_PORTS))
        port = self.registry.get("next_port", START_PORT)
        while port in assigned or port in reserved:
            port += 1
        return port

    def assign_port(
        self,
        service_name: str,
        force: bool = False,
        preferred_port: int | None = None,
    ) -> tuple[int, bool]:
        assignments = self.registry["assignments"]
        assigned = set(assignments.values())
        reserved = set(self.registry.get("reserved_ports", RESERVED_PORTS))
        if service_name in assignments and not force:
            return assignments[service_name], False
        if (
            preferred_port is not None
            and preferred_port not in assigned
            and preferred_port not in reserved
        ):
            port = preferred_port
        else:
            port = self._find_next_available_port()
        assignments[service_name] = port
        self.registry["next_port"] = max(self.registry.get("next_port", START_PORT), port + 1)
        self._save_registry()
        return port, True

    def release_port(self, service_name: str) -> int | None:
        assignments = self.registry["assignments"]
        if service_name not in assignments:
            return None
        port = assignments.pop(service_name)
        self._save_registry()
        return port

    def get_port(self, service_name: str) -> int | None:
        return self.registry["assignments"].get(service_name)

    def list_assignments(self) -> dict[str, int]:
        return self.registry["assignments"].copy()

    def update_config_files(self, service_name: str, port: int, port_only: bool = False) -> None:
        node_port = 31000 + (port - 8000)
        lay = self._layout
        values_file = self.project_root / lay["helm_values_dir"] / f"{service_name}.yaml"
        if not values_file.exists():
            print(f"Helm values file not found: {values_file}")
            print("   Create it with: rerp bootstrap microservice <service>")
            return
        with values_file.open() as f:
            values = yaml.safe_load(f) or {}
        if "service" not in values:
            values["service"] = {}
        values["service"]["name"] = service_name
        values["service"]["port"] = port
        values["service"]["containerPort"] = port
        values["service"]["nodePort"] = node_port
        if not port_only:
            if "image" not in values:
                values["image"] = {}
            values["image"]["name"] = f"rerp-{service_name}"
            if "app" not in values:
                values["app"] = {}
            values["app"]["serviceName"] = service_name
            values["app"]["binaryName"] = service_name.replace("-", "_") + "_service_api"
        with values_file.open("w") as f:
            yaml.dump(values, f, default_flow_style=False, sort_keys=False)
        print(f"Updated {values_file}")
        _update_openapi_servers(service_name, port, self.project_root, lay)
        if port_only:
            return
        kind_config = self.project_root / lay["kind_config"]
        if TILT_MANAGED_RANGE[0] <= port <= TILT_MANAGED_RANGE[1]:
            print(
                f"Info:  Skipping kind-config: port {port} in Tilt-managed range "
                f"{TILT_MANAGED_RANGE[0]}-{TILT_MANAGED_RANGE[1]} (Tilt port-forwards)"
            )
        elif kind_config.exists():
            with kind_config.open() as f:
                content = f.read()
            if f"hostPort: {port}" not in content:
                lines = content.split("\n")
                insert_index = None
                for i, line in enumerate(lines):
                    if "extraPortMappings:" in line:
                        j = i + 1
                        while j < len(lines) and (
                            lines[j].strip().startswith("#")
                            or lines[j].strip().startswith("-")
                            or "containerPort" in lines[j]
                            or "hostPort" in lines[j]
                            or "protocol" in lines[j]
                        ):
                            j += 1
                        insert_index = j
                        break
                if insert_index is not None:
                    indent = "  "
                    new_lines = [
                        indent + f"# {service_name.replace('-', ' ').title()} Service",
                        indent + f"- containerPort: {node_port}",
                        indent + f"  hostPort: {port}",
                        indent + "  protocol: TCP",
                    ]
                    lines[insert_index:insert_index] = new_lines
                    with kind_config.open("w") as f:
                        f.write("\n".join(lines))
                    print(f"Updated {kind_config}")
            else:
                print("Info:  Port mapping already exists in kind-config.yaml")
