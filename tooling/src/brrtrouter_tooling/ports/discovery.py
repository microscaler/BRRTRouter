"""Port discovery from helm, kind-config, Tiltfile, bff-suite-config, openapi (RERP-style layout)."""

from __future__ import annotations

import logging
import re
from collections.abc import Iterator
from pathlib import Path

import yaml

from brrtrouter_tooling.ports.layout import resolve_layout

log = logging.getLogger(__name__)


def _openapi_dir(project_root: Path, layout: dict[str, str]) -> Path:
    return project_root / layout["openapi_dir"]


def suites_with_bff(project_root: Path, layout: dict[str, str] | None = None) -> list[str]:
    """Suites that have a BFF: openapi/{suite}/bff-suite-config.yaml exists."""
    lay = resolve_layout(layout)
    d = _openapi_dir(project_root, lay)
    if not d.exists():
        return []
    name = lay["bff_suite_config_name"]
    return [x.name for x in d.iterdir() if x.is_dir() and (x / name).exists()]


def bff_suite_config_path(
    project_root: Path, suite: str, layout: dict[str, str] | None = None
) -> Path:
    lay = resolve_layout(layout)
    return _openapi_dir(project_root, lay) / suite / lay["bff_suite_config_name"]


def openapi_bff_path(project_root: Path, suite: str, layout: dict[str, str] | None = None) -> Path:
    lay = resolve_layout(layout)
    return _openapi_dir(project_root, lay) / suite / lay["openapi_bff_name"]


def service_to_suite(
    project_root: Path, service_name: str, layout: dict[str, str] | None = None
) -> str | None:
    """Return the suite that contains openapi/{suite}/{service_name}/openapi.yaml, or None."""
    lay = resolve_layout(layout)
    d = _openapi_dir(project_root, lay)
    if not d.exists():
        return None
    for x in d.iterdir():
        if x.is_dir() and (x / service_name / "openapi.yaml").exists():
            return x.name
    return None


def get_bff_service_name_from_config(data: dict) -> str | None:
    """Read bff_service_name from bff-suite-config (top-level or metadata). Returns None if absent."""
    return data.get("bff_service_name") or (data.get("metadata") or {}).get("bff_service_name")


def iter_bffs(
    project_root: Path, layout: dict[str, str] | None = None
) -> Iterator[tuple[str, str]]:
    """Yield (bff_service_name, suite) for each suite with bff-suite-config and bff_service_name set."""
    for suite in suites_with_bff(project_root, layout):
        path = bff_suite_config_path(project_root, suite, layout)
        try:
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            name = get_bff_service_name_from_config(data)
            if name:
                yield (name, suite)
        except (OSError, yaml.YAMLError, KeyError, TypeError, ValueError) as e:
            log.debug("Could not parse bff_suite_config %s: %s", path, e)


def bff_service_to_suite(
    project_root: Path, service_name: str, layout: dict[str, str] | None = None
) -> str | None:
    """Return the suite whose BFF has the given registry service name, or None."""
    for bff_svc, suite in iter_bffs(project_root, layout):
        if bff_svc == service_name:
            return suite
    return None


def load_suite_services(project_root: Path, layout: dict[str, str] | None = None) -> set[str]:
    """Services that keep their port in fix-duplicates: BFF names and bff-suite-config services."""
    keep: set[str] = set()
    for bff_svc, _ in iter_bffs(project_root, layout):
        keep.add(bff_svc)
    for suite in suites_with_bff(project_root, layout):
        path = bff_suite_config_path(project_root, suite, layout)
        try:
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            keep |= set((data.get("services") or {}).keys())
        except (OSError, yaml.YAMLError, KeyError, TypeError, ValueError) as e:
            log.debug("Could not parse bff_suite_config %s: %s", path, e)
    return keep


def discover_helm(project_root: Path, layout: dict[str, str] | None = None) -> dict[str, int]:
    """Discover service.port from helm values *.yaml."""
    out: dict[str, int] = {}
    lay = resolve_layout(layout)
    d = project_root / lay["helm_values_dir"]
    if not d.exists():
        return out
    for p in sorted(d.glob("*.yaml")):
        try:
            with p.open() as f:
                data = yaml.safe_load(f) or {}
            svc = data.get("service") or {}
            port = svc.get("port")
            name = svc.get("name") or p.stem
            if port is not None:
                out[name] = int(port)
        except Exception as e:  # noqa: BLE001
            log.warning("Could not parse helm values %s: %s", p, e)
    return out


def discover_kind_host_ports(
    project_root: Path, layout: dict[str, str] | None = None
) -> list[tuple[int, str]]:
    """Discover hostPort from kind-config extraPortMappings. Returns [(host_port, comment_or_container_port)]."""
    out: list[tuple[int, str]] = []
    lay = resolve_layout(layout)
    p = project_root / lay["kind_config"]
    if not p.exists():
        return out
    try:
        with p.open() as f:
            data = yaml.safe_load(f) or {}
        for n in data.get("nodes") or []:
            for m in n.get("extraPortMappings") or []:
                hp = m.get("hostPort")
                if hp is not None:
                    cp = m.get("containerPort", "")
                    out.append((int(hp), str(cp)))
    except Exception as e:  # noqa: BLE001
        log.warning("Could not parse kind-config %s: %s", p, e)
    return out


def discover_tiltfile(project_root: Path, layout: dict[str, str] | None = None) -> dict[str, int]:
    """Discover get_service_port dict from Tiltfile (starlark)."""
    out: dict[str, int] = {}
    lay = resolve_layout(layout)
    p = project_root / lay["tiltfile"]
    if not p.exists():
        return out
    try:
        with p.open() as f:
            text = f.read()
        m = re.search(r"ports\s*=\s*\{([^}]+)\}", text, re.DOTALL)
        if m:
            for m2 in re.finditer(r"'([^']+)'\s*:\s*'(\d+)'", m.group(1)):
                out[m2.group(1)] = int(m2.group(2))
    except Exception as e:  # noqa: BLE001
        log.warning("Could not parse Tiltfile %s: %s", p, e)
    return out


def discover_bff_suite_config(
    project_root: Path, layout: dict[str, str] | None = None
) -> dict[str, int]:
    """Discover services.*.port from all openapi/{suite}/bff-suite-config.yaml."""
    out: dict[str, int] = {}
    for suite in suites_with_bff(project_root, layout):
        path = bff_suite_config_path(project_root, suite, layout)
        try:
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            for name, cfg in (data.get("services") or {}).items():
                if isinstance(cfg, dict) and "port" in cfg:
                    out[name] = int(cfg["port"])
        except Exception as e:  # noqa: BLE001
            log.warning("Could not parse bff_suite_config %s: %s", path, e)
    return out


def discover_openapi_bff_localhost(
    project_root: Path, layout: dict[str, str] | None = None
) -> dict[str, tuple[int, str]]:
    """Extract localhost port from openapi/{suite}/openapi_bff.yaml per BFF. Returns {bff_service_name: (port, suite)}."""
    out: dict[str, tuple[int, str]] = {}
    for bff_svc, suite in iter_bffs(project_root, layout):
        path = openapi_bff_path(project_root, suite, layout)
        if not path.exists():
            continue
        try:
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            for s in data.get("servers") or []:
                if isinstance(s, dict):
                    u = str(s.get("url", ""))
                    if "localhost" in u:
                        m = re.search(r":(\d+)(?:/|$)", u)
                        if m:
                            out[bff_svc] = (int(m.group(1)), suite)
                        break
        except (
            OSError,
            yaml.YAMLError,
            KeyError,
            TypeError,
            ValueError,
            re.error,
        ) as e:
            log.debug("Could not parse openapi_bff %s: %s", path, e)
    return out


def discover_openapi_suite_microservice_localhost(
    project_root: Path, layout: dict[str, str] | None = None
) -> dict[str, tuple[str, int]]:
    """Extract localhost ports from openapi/{suite}/{name}/openapi.yaml. Returns {service_name: (suite, port)}."""
    out: dict[str, tuple[str, int]] = {}
    lay = resolve_layout(layout)
    d = _openapi_dir(project_root, lay)
    if not d.exists():
        return out
    for suite_dir in d.iterdir():
        if not suite_dir.is_dir():
            continue
        for sdir in suite_dir.iterdir():
            if not sdir.is_dir():
                continue
            spec = sdir / "openapi.yaml"
            if not spec.exists():
                continue
            try:
                with spec.open() as f:
                    data = yaml.safe_load(f) or {}
                for s in data.get("servers") or []:
                    if isinstance(s, dict):
                        u = str(s.get("url", ""))
                        if "localhost" in u:
                            m = re.search(r":(\d+)(?:/|$)", u)
                            if m:
                                out[sdir.name] = (suite_dir.name, int(m.group(1)))
                            break
            except (
                OSError,
                yaml.YAMLError,
                KeyError,
                TypeError,
                ValueError,
                re.error,
            ) as e:
                log.debug("Could not parse openapi %s: %s", spec, e)
    return out
