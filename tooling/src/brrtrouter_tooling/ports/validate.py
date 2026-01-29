"""Port validation: scan registry, helm, kind, Tiltfile, bff-suite-config; report conflicts."""

from __future__ import annotations

import json
import logging
import re
from collections import defaultdict
from pathlib import Path
from typing import Any

import yaml

from brrtrouter_tooling.ports.discovery import (
    bff_suite_config_path,
    discover_bff_suite_config,
    discover_helm,
    discover_kind_host_ports,
    discover_openapi_bff_localhost,
    discover_openapi_suite_microservice_localhost,
    discover_tiltfile,
    get_bff_service_name_from_config,
    iter_bffs,
    load_suite_services,
    suites_with_bff,
)
from brrtrouter_tooling.ports.registry import TILT_MANAGED_RANGE, PortRegistry

log = logging.getLogger(__name__)


def _validate_helm_duplicates(helm: dict[str, int], errors: list[str]) -> None:
    by_port: dict[int, list[str]] = defaultdict(list)
    for svc, port in helm.items():
        by_port[port].append(svc)
    for port, svcs in by_port.items():
        if len(svcs) > 1:
            errors.append(
                f"Duplicate service.port {port} in helm values: {', '.join(sorted(svcs))}"
            )


def _validate_kind_ports(
    kind_ports: list[tuple[int, str]], errors: list[str], warnings: list[str]
) -> None:
    kind_host = [p for p, _ in kind_ports]
    if len(kind_host) != len(set(kind_host)):
        seen: set[int] = set()
        for p, _ in kind_ports:
            if p in seen:
                errors.append("Duplicate hostPort in kind-config.yaml")
            seen.add(p)
    for hp, _ in kind_ports:
        if TILT_MANAGED_RANGE[0] <= hp <= TILT_MANAGED_RANGE[1]:
            warnings.append(
                f"kind-config hostPort {hp} is in Tilt-managed range "
                f"{TILT_MANAGED_RANGE[0]}-{TILT_MANAGED_RANGE[1]}; "
                "Tilt port-forwards also bind these. Remove from kind extraPortMappings."
            )


def _validate_registry_helm_tilt_bff(
    reg: dict[str, int],
    helm: dict[str, int],
    tilt: dict[str, int],
    bff: dict[str, int],
    errors: list[str],
    warnings: list[str],
) -> None:
    for svc, port in helm.items():
        r = reg.get(svc)
        if r is not None and r != port:
            errors.append(f"Port mismatch: registry has {svc}={r}, helm has {port}")
    for svc, port in tilt.items():
        r = reg.get(svc)
        if r is not None and r != port:
            errors.append(f"Port mismatch: registry has {svc}={r}, Tiltfile has {port}")
    for svc, port in helm.items():
        t = tilt.get(svc)
        if t is not None and t != port:
            errors.append(f"Port mismatch: helm has {svc}={port}, Tiltfile has {t}")
    for svc, port in bff.items():
        if helm.get(svc) is not None and helm[svc] != port:
            warnings.append(f"bff-suite-config {svc}= {port} differs from helm {helm[svc]}")
        if tilt.get(svc) is not None and tilt[svc] != port:
            warnings.append(f"bff-suite-config {svc}= {port} differs from Tiltfile {tilt[svc]}")


def _validate_bff_suite_configs(
    project_root: Path,
    layout: dict[str, Any] | None,
    reg: dict[str, int],
    errors: list[str],
    warnings: list[str],
) -> None:
    for suite in suites_with_bff(project_root, layout):
        try:
            path = bff_suite_config_path(project_root, suite, layout)
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            if not get_bff_service_name_from_config(data):
                warnings.append(
                    f"openapi/{suite}/bff-suite-config.yaml has no bff_service_name. "
                    "Add it so ports can discover this suite's BFF."
                )
        except (OSError, yaml.YAMLError, KeyError, TypeError, ValueError) as e:
            log.debug("Could not read bff_suite_config %s: %s", suite, e)

    for bff_svc, suite in iter_bffs(project_root, layout):
        path = bff_suite_config_path(project_root, suite, layout)
        if not path.exists() or bff_svc not in reg:
            continue
        try:
            with path.open() as f:
                bsc = yaml.safe_load(f) or {}
            for s in (bsc.get("metadata") or {}).get("servers") or []:
                if isinstance(s, dict) and "localhost" in str(s.get("url", "")):
                    m = re.search(r":(\d+)(?:/|$)", str(s.get("url", "")))
                    if m and int(m.group(1)) != reg[bff_svc]:
                        errors.append(
                            f"openapi/{suite}/bff-suite-config.yaml metadata.servers localhost port "
                            f"{m.group(1)} differs from registry {bff_svc}={reg[bff_svc]}. "
                            f"Run: rerp ports update-configs {bff_svc}"
                        )
                    break
        except (OSError, yaml.YAMLError, KeyError, TypeError, ValueError, re.error) as e:
            log.debug("Could not read bff_suite_config %s: %s", path, e)


def _validate_openapi_localhost(
    project_root: Path,
    layout: dict[str, Any] | None,
    reg: dict[str, int],
    errors: list[str],
) -> None:
    obff = discover_openapi_bff_localhost(project_root, layout)
    for bff_svc, (port, suite) in obff.items():
        if bff_svc not in reg or reg[bff_svc] == port:
            continue
        errors.append(
            f"openapi/{suite}/openapi_bff.yaml localhost server port {port} differs from registry "
            f"{bff_svc}={reg[bff_svc]}. Regenerate BFF spec."
        )

    oacc = discover_openapi_suite_microservice_localhost(project_root, layout)
    for svc, (suite, port) in oacc.items():
        r = reg.get(svc)
        if r is not None and r != port:
            errors.append(
                f"openapi/{suite}/{svc}/openapi.yaml localhost server port {port} differs from "
                f"registry {svc}={r}. Run: rerp ports update-configs {svc}"
            )


def validate(
    registry: PortRegistry,
    project_root: Path,
    json_out: bool = False,
    layout: dict[str, Any] | None = None,
) -> int:
    """Scan registry, helm, kind, Tiltfile, bff-suite-config; report conflicts. Return 0 if ok, 1 if conflicts."""
    reg = registry.list_assignments()
    helm = discover_helm(project_root, layout)
    kind_ports = discover_kind_host_ports(project_root, layout)
    tilt = discover_tiltfile(project_root, layout)
    bff = discover_bff_suite_config(project_root, layout)

    errors: list[str] = []
    warnings: list[str] = []

    _validate_helm_duplicates(helm, errors)
    _validate_kind_ports(kind_ports, errors, warnings)
    _validate_registry_helm_tilt_bff(reg, helm, tilt, bff, errors, warnings)
    _validate_bff_suite_configs(project_root, layout, reg, errors, warnings)
    _validate_openapi_localhost(project_root, layout, reg, errors)

    if json_out:
        print(
            json.dumps(
                {"ok": len(errors) == 0, "errors": errors, "warnings": warnings},
                indent=2,
            )
        )
        return 0 if len(errors) == 0 else 1

    for e in errors:
        print(f"Error: {e}")
    for w in warnings:
        print(f"Warning: {w}")
    if errors:
        print("\nRun: brrtrouter ports list   and   brrtrouter ports validate")
        return 1
    if warnings:
        print("\nNo hard conflicts; see warnings above.")
        return 0
    print("No port conflicts found.")
    return 0


def reconcile(
    registry: PortRegistry,
    project_root: Path,
    update_configs: bool = False,
    layout: dict[str, Any] | None = None,
) -> int:
    """Add helm-only services to registry (using helm port)."""
    from brrtrouter_tooling.ports.registry import RESERVED_PORTS

    helm = discover_helm(project_root, layout)
    reg = registry.list_assignments()
    assigned_ports = set(reg.values())
    reserved = set(registry.registry.get("reserved_ports", RESERVED_PORTS))
    for name, port in sorted(helm.items()):
        if name not in reg:
            if port in assigned_ports or port in reserved:
                print(
                    f"Warning: {name}: helm has port {port} but it is already taken; skip adding."
                )
                continue
            registry.assign_port(name, force=False, preferred_port=port)
            print(f"Added to registry: {name} = {port}")
            assigned_ports.add(port)
            if update_configs:
                registry.update_config_files(name, port)
        else:
            if reg[name] != port:
                print(f"Warning: {name}: registry={reg[name]}, helm={port} (no change)")
    return 0


def fix_duplicates(
    registry: PortRegistry,
    project_root: Path,
    dry_run: bool = False,
    layout: dict[str, Any] | None = None,
) -> int:
    """Resolve duplicate service.port in helm; prefer suite (BFF + bff-suite-config)."""
    helm = discover_helm(project_root, layout)
    by_port: dict[int, list[str]] = defaultdict(list)
    for svc, port in helm.items():
        by_port[port].append(svc)
    suite_keepers = load_suite_services(project_root, layout)
    reg = registry.list_assignments()
    dupes = [(p, svcs) for p, svcs in sorted(by_port.items()) if len(svcs) > 1]
    if not dupes:
        print("No duplicate helm ports found.")
        return 0
    print(f"Resolving {len(dupes)} duplicate port(s)...")
    for port, svcs in dupes:
        in_keep = [s for s in svcs if s in suite_keepers]
        keeper = sorted(in_keep)[0] if in_keep else sorted(svcs)[0]
        losers = [s for s in svcs if s != keeper]
        need_p_for_k = reg.get(keeper) != port
        if need_p_for_k:
            for s in svcs:
                if s != keeper and reg.get(s) == port:
                    if not dry_run:
                        registry.release_port(s)
                        reg = registry.list_assignments()
                    print(f"  release {s} (free {port} for {keeper})")
            if not dry_run:
                registry.assign_port(keeper, force=False, preferred_port=port)
                registry.update_config_files(keeper, port, port_only=True)
                reg = registry.list_assignments()
            print(f"  assign {keeper} = {port}")
        for loser in losers:
            if reg.get(loser) is not None:
                if not dry_run:
                    registry.release_port(loser)
                    reg = registry.list_assignments()
                print(f"  release {loser}")
            if not dry_run:
                p2, _ = registry.assign_port(loser, force=False, preferred_port=None)
                registry.update_config_files(loser, p2, port_only=True)
                reg = registry.list_assignments()
                print(f"  assign {loser} = {p2}  (update helm)")
            else:
                print(f"  assign {loser} = (next available)")
    if dry_run:
        print("  (dry-run; no changes written)")
    return 0
