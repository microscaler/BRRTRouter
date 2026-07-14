"""Suite and BFF discovery from openapi/ (flat or nested suite layout)."""

import logging
from collections.abc import Iterator
from pathlib import Path

import yaml

log = logging.getLogger(__name__)


def _openapi_dir(project_root: Path) -> Path:
    return project_root / "openapi"


def _microservices_dir(project_root: Path) -> Path:
    return project_root / "microservices"


def _nested_service_openapi(project_root: Path, suite: str, service_name: str) -> Path:
    return _openapi_dir(project_root) / suite / service_name / "openapi.yaml"


def _flat_service_openapi(project_root: Path, service_name: str) -> Path:
    return _openapi_dir(project_root) / service_name / "openapi.yaml"


def _nested_bff_openapi(project_root: Path, suite: str) -> Path:
    return _openapi_dir(project_root) / suite / "openapi_bff.yaml"


def _flat_bff_openapi(project_root: Path) -> Path:
    return _openapi_dir(project_root) / "openapi_bff.yaml"


def _nested_microservice_dir(project_root: Path, suite: str, service_name: str) -> Path:
    return _microservices_dir(project_root) / suite / service_name


def _flat_microservice_dir(project_root: Path, service_name: str) -> Path:
    return _microservices_dir(project_root) / service_name


def _first_existing(*paths: Path) -> Path | None:
    for path in paths:
        if path.exists():
            return path
    return None


def project_uses_flat_openapi_layout(project_root: Path) -> bool:
    """True when openapi/{service}/openapi.yaml exists (legacy hauliage layout)."""
    d = _openapi_dir(project_root)
    if not d.is_dir():
        return False
    return any(child.is_dir() and (child / "openapi.yaml").is_file() for child in d.iterdir())


def project_uses_flat_microservice_layout(project_root: Path) -> bool:
    """True when microservices/{service}/ exists without a suite prefix."""
    ms = _microservices_dir(project_root)
    if not ms.is_dir():
        return False
    for child in ms.iterdir():
        if not child.is_dir():
            continue
        if (child / "gen").is_dir() or (child / "impl").is_dir():
            return True
    return False


def resolve_service_openapi_spec_path(project_root: Path, suite: str, service_name: str) -> Path:
    """Resolve OpenAPI spec path; prefers existing flat or nested layout on disk."""
    if bff_service_to_suite(project_root, service_name) == suite:
        found = _first_existing(
            _flat_bff_openapi(project_root),
            _nested_bff_openapi(project_root, suite),
        )
        if found is not None:
            return found
        # Target layout is nested; flat remains for legacy hauliage BFF.
        return (
            _flat_bff_openapi(project_root)
            if project_uses_flat_openapi_layout(project_root)
            else _nested_bff_openapi(project_root, suite)
        )

    found = _first_existing(
        _nested_service_openapi(project_root, suite, service_name),
        _flat_service_openapi(project_root, service_name),
    )
    if found is not None:
        return found
    if project_uses_flat_openapi_layout(project_root):
        return _flat_service_openapi(project_root, service_name)
    return _nested_service_openapi(project_root, suite, service_name)


def resolve_service_microservice_dir(project_root: Path, suite: str, service_name: str) -> Path:
    """Resolve microservices crate root; prefers existing flat or nested layout on disk."""
    found = _first_existing(
        _nested_microservice_dir(project_root, suite, service_name),
        _flat_microservice_dir(project_root, service_name),
    )
    if found is not None:
        return found
    if project_uses_flat_microservice_layout(project_root):
        return _flat_microservice_dir(project_root, service_name)
    return _nested_microservice_dir(project_root, suite, service_name)


# Back-compat aliases used by bootstrap/regenerate callers.
def service_openapi_spec_path(project_root: Path, suite: str, service_name: str) -> Path:
    return resolve_service_openapi_spec_path(project_root, suite, service_name)


def service_microservice_dir(project_root: Path, suite: str, service_name: str) -> Path:
    return resolve_service_microservice_dir(project_root, suite, service_name)


def suites_with_bff(project_root: Path) -> list[str]:
    """Suites that have a BFF: openapi/bff-suite-config.yaml exists."""
    d = _openapi_dir(project_root)
    if not d.exists():
        return []
    return ["hauliage"] if (d / "bff-suite-config.yaml").exists() else []


def bff_suite_config_path(project_root: Path, suite: str) -> Path:
    return _openapi_dir(project_root) / "bff-suite-config.yaml"


def openapi_bff_path(project_root: Path, suite: str) -> Path:
    found = _first_existing(
        _flat_bff_openapi(project_root),
        _nested_bff_openapi(project_root, suite),
    )
    if found is not None:
        return found
    if project_uses_flat_openapi_layout(project_root):
        return _flat_bff_openapi(project_root)
    return _nested_bff_openapi(project_root, suite)


def service_to_suite(project_root: Path, service_name: str) -> str | None:
    """Return suite for a service if its OpenAPI spec exists (flat or nested)."""
    d = _openapi_dir(project_root)
    if not d.exists():
        return None
    if (d / service_name / "openapi.yaml").exists():
        return "hauliage"
    for suite_dir in d.iterdir():
        if suite_dir.is_dir() and (suite_dir / service_name / "openapi.yaml").exists():
            return suite_dir.name
    return None


def get_bff_service_name_from_config(data: dict) -> str | None:
    """Read bff_service_name from bff-suite-config (top-level or metadata)."""
    return data.get("bff_service_name") or (data.get("metadata") or {}).get("bff_service_name")


def iter_bffs(project_root: Path, suite: str | None = None) -> Iterator[tuple[str, str]]:
    """Yield (bff_service_name, suite) for each suite with bff-suite-config.yaml."""
    suites = [suite] if suite is not None else suites_with_bff(project_root)
    for s in suites:
        if suite is not None and s != suite:
            continue
        path = bff_suite_config_path(project_root, s)
        if not path.exists():
            continue
        try:
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            name = get_bff_service_name_from_config(data)
            if name:
                yield (name, s)
        except (OSError, yaml.YAMLError, KeyError, TypeError, ValueError) as e:
            log.debug("Could not parse bff_suite_config %s: %s", path, e)


def bff_service_to_suite(project_root: Path, service_name: str) -> str | None:
    """Return the suite whose BFF has the given registry service name, or None."""
    for bff_svc, suite in iter_bffs(project_root):
        if bff_svc == service_name:
            return suite
    return None


def load_suite_services(project_root: Path) -> set:
    """Services that keep their port in fix-duplicates: BFF names and bff-suite-config."""
    keep: set = set()
    for bff_svc, _ in iter_bffs(project_root):
        keep.add(bff_svc)
    for suite in suites_with_bff(project_root):
        path = bff_suite_config_path(project_root, suite)
        try:
            with path.open() as f:
                data = yaml.safe_load(f) or {}
            keep |= set((data.get("services") or {}).keys())
        except (OSError, yaml.YAMLError, KeyError, TypeError, ValueError) as e:
            log.debug("Could not parse bff_suite_config %s: %s", path, e)
    return keep


def suite_sub_service_names(project_root: Path, suite: str) -> list[str]:
    """Return sorted sub-service names from supported OpenAPI layouts.

    Both ``openapi/{service}/openapi.yaml`` and
    ``openapi/{suite}/{service}/openapi.yaml`` are discovered.
    """
    d = _openapi_dir(project_root)
    if not d.is_dir():
        return []
    names: set[str] = set()
    for child in d.iterdir():
        if child.is_dir() and (child / "openapi.yaml").is_file():
            names.add(child.name)
    nested = d / suite
    if nested.is_dir():
        for child in nested.iterdir():
            if child.is_dir() and (child / "openapi.yaml").is_file():
                names.add(child.name)
    return sorted(names)


def iter_suite_services(project_root: Path, suite: str | None = None) -> Iterator[tuple[str, str]]:
    """Yield (suite, service_name) for discovered OpenAPI sub-services."""
    if suite is None or suite == "hauliage":
        for name in suite_sub_service_names(project_root, "hauliage"):
            yield ("hauliage", name)


def tilt_service_names(project_root: Path) -> list[str]:
    """Service names that Tilt runs. From bff-suite-config + BFFs."""
    return sorted(load_suite_services(project_root))
