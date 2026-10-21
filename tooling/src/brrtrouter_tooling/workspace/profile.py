"""The consuming project's description of itself: names, layout and dev settings.

The tooling never names a project. Everything project-specific comes from a ``ProjectProfile`` loaded once from the
project root:

* ``brrtrouter.toml`` at the project root, or
* ``[tool.brrtrouter]`` in ``tooling/pyproject.toml`` (same tables),

and every field not given is derived from ``project.name``, which defaults to the project directory's name.

.. code-block:: toml

    [project]
    name = "acme"                    # default: the project directory's name
    cli = "acme"                     # the console script's name in usage and help

    [naming]
    package_prefix = "acme"
    crate_pattern = "{prefix}_{service}"     # crates: acme_market, acme_market_gen; "{prefix}_{suite}_{service}"
                                             # for suite-qualified crates (acme_accounting_ledger)
    binary_pattern = "{prefix}_{system}_{module}_impl"
    image_prefix = "acme"
    image_pattern = "{prefix}-{service}"     # images: acme-market; "{prefix}-{suite}-{service}" for suite-qualified
    base_image = "acme-base"
    zip_prefix = "acme-binaries-"

    [layout]
    openapi_dir = "openapi"
    workspace_dir = "microservices"
    helm_values = "helm/acme-microservice/values"
    flat_suite = "acme"              # the suite a flat openapi/<service>/ layout belongs to
    default_suite = "acme"           # the suite bootstrap uses when none is given

    [dev]
    kind_cluster = "acme"
    tilt_port = 10350
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from functools import lru_cache
from pathlib import Path
from typing import Any

if sys.version_info >= (3, 11):
    import tomllib
else:  # pragma: no cover - exercised on 3.10 only
    import tomli as tomllib

PROFILE_FILE = "brrtrouter.toml"


def _slug(name: str) -> str:
    """A project directory name as a project name: lower case, runs of other characters as one '-'."""
    s = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    return s or "app"


@dataclass(frozen=True)
class ProjectProfile:
    """What the tooling needs to know about the project it runs in. Build with ``load_profile``."""

    root: Path
    name: str
    cli: str
    package_prefix: str
    crate_pattern: str
    binary_pattern: str
    image_prefix: str
    image_pattern: str
    base_image: str
    zip_prefix: str
    openapi_dir: str
    workspace_dir: str
    helm_values: str
    flat_suite: str
    default_suite: str
    kind_cluster: str
    tilt_port: int
    source: str = field(default="defaults", compare=False)

    @property
    def snake(self) -> str:
        """The name as an identifier part (``acme-erp`` -> ``acme_erp``)."""
        return self.name.replace("-", "_")

    def binary_name(self, system: str, module: str) -> str:
        """Impl binary name for a service, from ``binary_pattern``."""
        return self.binary_pattern.format(
            prefix=self.package_prefix, system=system, module=module.replace("-", "_")
        )

    def image_name(self, suite: str, service: str) -> str:
        """Container image name for a service, from ``naming.image_pattern`` (default ``{prefix}-{service}``).

        The one image-name contract: Helm values (``ports update-configs``), Tilt teardown and the docker commands all
        use it, so they cannot disagree.
        """
        return self.image_pattern.format(prefix=self.image_prefix, suite=suite, service=service)

    def crate_prefix(self, suite: str) -> str:
        """The crate-name prefix for a suite's services: ``crate_pattern`` up to ``_{service}``.

        ``{prefix}_{service}`` gives ``acme`` (crates ``acme_market``); ``{prefix}_{suite}_{service}`` gives
        ``acme_accounting`` (crates ``acme_accounting_ledger``). Bootstrap, discovery and the single-crate build all
        name crates through this.
        """
        head = self.crate_pattern[: -len("_{service}")]
        return head.format(prefix=self.package_prefix, suite=suite.replace("-", "_"))

    def crate_name(self, suite: str, service: str) -> str:
        """A service's impl crate name; its gen crate is this plus ``_gen``."""
        return f"{self.crate_prefix(suite)}_{service.replace('-', '_')}"

    @property
    def helm_values_dir(self) -> Path:
        return self.root / self.helm_values

    @property
    def openapi_path(self) -> Path:
        return self.root / self.openapi_dir

    @property
    def workspace_path(self) -> Path:
        return self.root / self.workspace_dir


def _read_tables(root: Path) -> tuple[dict[str, Any], str]:
    f = root / PROFILE_FILE
    if f.is_file():
        return tomllib.loads(f.read_text()), str(f)
    py = root / "tooling" / "pyproject.toml"
    if py.is_file():
        data = tomllib.loads(py.read_text())
        tool = (data.get("tool") or {}).get("brrtrouter")
        if isinstance(tool, dict):
            return tool, f"{py} [tool.brrtrouter]"
    return {}, "defaults"


def profile_from_tables(
    root: Path, tables: dict[str, Any], source: str = "defaults"
) -> ProjectProfile:
    """Resolve a profile from parsed tables; every missing field is derived from ``project.name``."""
    project = tables.get("project") or {}
    naming = tables.get("naming") or {}
    layout = tables.get("layout") or {}
    dev = tables.get("dev") or {}
    name = str(project.get("name") or _slug(root.resolve().name))
    snake = name.replace("-", "_")
    prefix = str(naming.get("package_prefix") or snake)
    image_prefix = str(naming.get("image_prefix") or name)
    crate_pattern = str(naming.get("crate_pattern") or "{prefix}_{service}")
    if not crate_pattern.endswith("_{service}"):
        msg = f"naming.crate_pattern must end with '_{{service}}', got {crate_pattern!r}"
        raise ValueError(msg)
    return ProjectProfile(
        root=root,
        name=name,
        cli=str(project.get("cli") or name),
        package_prefix=prefix,
        crate_pattern=crate_pattern,
        binary_pattern=str(naming.get("binary_pattern") or "{prefix}_{system}_{module}_impl"),
        image_prefix=image_prefix,
        image_pattern=str(naming.get("image_pattern") or "{prefix}-{service}"),
        base_image=str(naming.get("base_image") or f"{image_prefix}-base"),
        zip_prefix=str(naming.get("zip_prefix") or f"{name}-binaries-"),
        openapi_dir=str(layout.get("openapi_dir") or "openapi"),
        workspace_dir=str(layout.get("workspace_dir") or "microservices"),
        helm_values=str(layout.get("helm_values") or f"helm/{name}-microservice/values"),
        flat_suite=str(layout.get("flat_suite") or name),
        default_suite=str(layout.get("default_suite") or layout.get("flat_suite") or name),
        kind_cluster=str(dev.get("kind_cluster") or name),
        tilt_port=int(dev.get("tilt_port") or 10350),
        source=source,
    )


@lru_cache(maxsize=32)
def _load(root: str) -> ProjectProfile:
    r = Path(root)
    tables, source = _read_tables(r)
    return profile_from_tables(r, tables, source)


def load_profile(project_root: Path | str) -> ProjectProfile:
    """The project's profile (cached per resolved root)."""
    return _load(str(Path(project_root).resolve()))


def clear_cache() -> None:
    """Forget loaded profiles (tests, or a long-lived process whose profile file changed)."""
    global _ACTIVE
    _load.cache_clear()
    _ACTIVE = None


_ACTIVE: ProjectProfile | None = None


def activate(profile: ProjectProfile) -> ProjectProfile:
    """Make ``profile`` the one this process's messages name (the CLI entry point calls it once)."""
    global _ACTIVE
    _ACTIVE = profile
    return profile


def cli_name() -> str:
    """The console script's name for usage and help: the active profile's ``project.cli``, else ``argv[0]``."""
    if _ACTIVE is not None:
        return _ACTIVE.cli
    return Path(sys.argv[0]).name or "brrtrouter"
