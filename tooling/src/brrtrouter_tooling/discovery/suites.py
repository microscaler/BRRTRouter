"""Suite and BFF discovery from openapi/{suite}/ and bff-suite-config.yaml."""

from __future__ import annotations

import logging
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import yaml

from brrtrouter_tooling.ports.layout import resolve_layout

log = logging.getLogger(__name__)


def _openapi_dir(project_root: Path, layout: dict[str, Any] | None = None) -> Path:
    cfg = resolve_layout(layout)
    return project_root / cfg["openapi_dir"]


def _bff_suite_config_name(layout: dict[str, Any] | None) -> str:
    return resolve_layout(layout)["bff_suite_config_name"]


def _openapi_bff_name(layout: dict[str, Any] | None) -> str:
    return resolve_layout(layout)["openapi_bff_name"]


def suites_with_bff(project_root: Path, layout: dict[str, Any] | None = None) -> list[str]:
    """Suites that have a BFF: openapi/{suite}/bff-suite-config.yaml exists."""
    d = _openapi_dir(project_root, layout)
    if not d.exists():
        return []
    config_name = _bff_suite_config_name(layout)
    return [x.name for x in d.iterdir() if x.is_dir() and (x / config_name).exists()]


def bff_suite_config_path(
    project_root: Path, suite: str, layout: dict[str, Any] | None = None
) -> Path:
    return _openapi_dir(project_root, layout) / suite / _bff_suite_config_name(layout)


def openapi_bff_path(project_root: Path, suite: str, layout: dict[str, Any] | None = None) -> Path:
    return _openapi_dir(project_root, layout) / suite / _openapi_bff_name(layout)


def service_to_suite(
    project_root: Path, service_name: str, layout: dict[str, Any] | None = None
) -> str | None:
    """Return the suite that contains openapi/{suite}/{service_name}/openapi.yaml, or None."""
    d = _openapi_dir(project_root, layout)
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
    project_root: Path, layout: dict[str, Any] | None = None
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
    project_root: Path, service_name: str, layout: dict[str, Any] | None = None
) -> str | None:
    """Return the suite whose BFF has the given registry service name, or None."""
    for bff_svc, suite in iter_bffs(project_root, layout):
        if bff_svc == service_name:
            return suite
    return None


def load_suite_services(project_root: Path, layout: dict[str, Any] | None = None) -> set[str]:
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


def suite_sub_service_names(
    project_root: Path, suite: str, layout: dict[str, Any] | None = None
) -> list[str]:
    """Sub-service names for a suite: openapi/{suite}/{name}/openapi.yaml exists. Sorted. Excludes BFF."""
    d = _openapi_dir(project_root, layout) / suite
    if not d.exists() or not d.is_dir():
        return []
    return sorted(x.name for x in d.iterdir() if x.is_dir() and (x / "openapi.yaml").exists())


def tilt_service_names(project_root: Path, layout: dict[str, Any] | None = None) -> list[str]:
    """Service names that Tilt runs. Sorted. From bff-suite-config + BFFs."""
    return sorted(load_suite_services(project_root, layout))
