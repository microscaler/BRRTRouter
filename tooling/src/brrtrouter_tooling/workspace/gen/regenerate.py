"""Regenerate services from OpenAPI specs (flat or nested suite layout)."""

import fnmatch
import sys
from pathlib import Path

from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate
from brrtrouter_tooling.workspace.build.constants import get_package_names
from brrtrouter_tooling.workspace.ci.fix_cargo_paths import run as run_fix_cargo_paths
from brrtrouter_tooling.workspace.discovery.suites import (
    resolve_service_microservice_dir,
    resolve_service_openapi_spec_path,
)
from brrtrouter_tooling.workspace.env_paths import discover_brrtrouter_root
from brrtrouter_tooling.workspace.profile import load_profile

if sys.version_info >= (3, 11):
    import tomllib
else:  # pragma: no cover - exercised on 3.10 only
    import tomli as tomllib


def workspace_members(project_root: Path) -> list[str] | None:
    """The ``[workspace] members`` entries of ``<layout.workspace_dir>/Cargo.toml``, as relative path patterns.

    Patterns, not expanded paths: ``*/gen`` or ``trader/*/gen`` must match a gen crate on a fresh clone, before the
    directory exists. None when there is no manifest or it lists no members; callers then fall back to "the gen
    directory exists" (never "nothing is a member").
    """
    manifest = load_profile(project_root).workspace_path / "Cargo.toml"
    if not manifest.is_file():
        return None
    try:
        data = tomllib.loads(manifest.read_text())
    except (OSError, tomllib.TOMLDecodeError):
        return None
    members = (data.get("workspace") or {}).get("members") or []
    out = [_norm(str(m)) for m in members if str(m).strip()]
    return out or None


def _norm(rel: str) -> str:
    rel = rel.strip().replace("\\", "/")
    while rel.startswith("./"):
        rel = rel[2:]
    return rel.rstrip("/")


def _is_member(rel: str, members: list[str], ws: Path) -> bool:
    """Whether a gen crate path is one of the workspace's crates.

    An explicit member names it, whether or not it exists yet (a fresh clone of a project that does not commit gen
    crates). A glob member (``trader/*/gen``) only names directories that are there, as Cargo reads it, so a match
    counts when the crate's directory exists or the service has its hand-written ``impl`` crate beside it: otherwise
    every proxied spec under the suite would match the glob and get a stray crate.
    """
    if rel in members:
        return True
    if not any(fnmatch.fnmatchcase(rel, m) for m in members if any(c in m for c in "*?[")):
        return False
    crate = ws / rel
    return crate.is_dir() or (crate.parent / "impl").is_dir()


def _gen_candidates(suite: str, service_name: str) -> list[str]:
    """Where a service's gen crate can live, relative to the workspace: nested first, then the flat layout."""
    return [f"{suite}/{service_name}/gen", f"{service_name}/gen"]


def service_gen_dir(
    project_root: Path, suite: str, service_name: str, members: list[str] | None = None
) -> Path | None:
    """The service's gen crate directory when it is one of the workspace's own crates, else None.

    With a workspace manifest: the first candidate (``{suite}/{service}/gen``, then ``{service}/gen``) that a member
    names (``_is_member``: explicit members whether or not they exist yet; glob members when the crate or its
    ``impl`` sibling is there). A gen directory no member names is a stray (nothing builds it), and
    a spec under ``openapi/{suite}/`` with no member is one the BFF only proxies. Without a manifest: the gen directory
    that exists on disk.
    """
    if members is None:
        members = workspace_members(project_root)
    if members is None:
        gen = resolve_service_microservice_dir(project_root, suite, service_name) / "gen"
        return gen if gen.is_dir() else None
    ws = load_profile(project_root).workspace_path
    for rel in _gen_candidates(suite, service_name):
        if _is_member(rel, members, ws):
            return ws / rel
    return None


def is_service_gen_crate(
    project_root: Path, suite: str, service_name: str, members: list[str] | None = None
) -> bool:
    """True when the service's gen crate is one of the workspace's own crates (``service_gen_dir``)."""
    return service_gen_dir(project_root, suite, service_name, members=members) is not None


def _fix_cargo_paths_callback(cargo_toml_path: Path, project_root: Path | None) -> None:
    run_fix_cargo_paths(cargo_toml_path, project_root=project_root)


def _gen_package_name(
    project_root: Path, service_name: str, suite: str | None = None
) -> str | None:
    """Gen crate [package].name for a service (e.g. acme_identity_gen).

    Scope discovery to the SUITE: with suite=None a service name that also
    exists in another openapi/ tree (e.g. a draft-tier sketch without a
    bff-suite-config) shadows the real one and picks up the fallback prefix.
    """
    impl_name = get_package_names(project_root, suite=suite).get(service_name)
    return f"{impl_name}_gen" if impl_name else None


def regenerate_service(
    project_root: Path,
    suite: str,
    service_name: str,
    brrtrouter_path: Path | None = None,
    allow_new: bool = False,
    output_dir: Path | None = None,
) -> int:
    """Regenerate a single service. Returns 0 on success, 1 on error. ``output_dir`` overrides where the crate is
    written (``gen_if_missing`` passes the member path it matched).

    Only a service that already has a gen crate is regenerated. A spec under ``openapi/{suite}/`` is not always a
    service of that suite: a BFF merges specs it only proxies (another suite's service, an external API), and
    generating those writes a stray crate. Creating a new service's first gen crate is explicit: ``allow_new``.
    """
    own = service_gen_dir(project_root, suite, service_name)
    if output_dir is None:
        output_dir = (
            own or resolve_service_microservice_dir(project_root, suite, service_name) / "gen"
        )
    if not allow_new and own is None:
        why = (
            "it is not a workspace member (a stray crate)"
            if output_dir.is_dir()
            else "its spec may only be proxied by the BFF"
        )
        print(
            f"❌ No gen crate for '{service_name}' in suite '{suite}' at {output_dir}: {why}. "
            "Pass --new to create its first gen crate."
        )
        return 1
    package_name = _gen_package_name(project_root, service_name, suite=suite)

    if brrtrouter_path is None:
        brrtrouter_path = discover_brrtrouter_root(project_root)

    spec_path = resolve_service_openapi_spec_path(project_root, suite, service_name)
    deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"

    if not spec_path.exists():
        print(f"❌ OpenAPI spec not found: {spec_path}")
        return 1

    try:
        result = call_brrtrouter_generate(
            spec_path=spec_path,
            output_dir=output_dir,
            project_root=project_root,
            brrtrouter_path=brrtrouter_path,
            deps_config_path=deps_config_path if deps_config_path.exists() else None,
            package_name=package_name,
            capture_output=False,
        )

        if result.returncode != 0:
            print(f"❌ Failed to regenerate {service_name}")
            return 1

        print(f"✅ Regenerated {service_name}")

        gen_cargo = output_dir / "Cargo.toml"
        if gen_cargo.exists():
            _fix_cargo_paths_callback(gen_cargo, project_root)

        return 0
    except FileNotFoundError as e:
        print(f"❌ {e}")
        return 1


def existing_gen_services(
    project_root: Path, suite: str, service_names: list[str]
) -> tuple[list[str], list[str]]:
    """Split a suite's spec names into (its own services, specs it only proxies).

    A service is one whose gen crate is a workspace member (``is_service_gen_crate``).
    """
    members = workspace_members(project_root)
    have, proxied = [], []
    for name in service_names:
        own = is_service_gen_crate(project_root, suite, name, members=members)
        (have if own else proxied).append(name)
    return have, proxied


def regenerate_suite_services(
    project_root: Path,
    suite: str,
    service_names: list[str],
    brrtrouter_path: Path | None = None,
) -> int:
    """Regenerate all services in a suite. Returns 0 if all succeed, 1 if any fail."""
    failed = []
    for service_name in service_names:
        rc = regenerate_service(
            project_root,
            suite,
            service_name,
            brrtrouter_path=brrtrouter_path,
        )
        if rc != 0:
            failed.append(service_name)

    if failed:
        print(f"\n❌ Failed to regenerate {len(failed)} service(s): {', '.join(failed)}")
        return 1

    print(f"\n✅ Successfully regenerated {len(service_names)} service(s) in suite '{suite}'")
    return 0
