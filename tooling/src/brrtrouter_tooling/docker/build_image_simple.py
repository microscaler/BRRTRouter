"""Build Docker image and push (or kind load)."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def _push_oci_tag(root: Path, tag: str, *, attempts: int = 3) -> subprocess.CompletedProcess[str]:
    """Push a local image tag to its registry as OCI (zot-friendly).

    Plain ``docker push`` stores Docker schema2 manifests; zot often rejects
    those (415) or later digest-mismatches when BuildKit rewrites the same
    blobs as OCI for a ``:dev-*`` retag. Pushing ``:tilt`` via buildx with
    ``oci-mediatypes=true`` keeps the whole Flux publish path on OCI.

    Retries ``provided digest did not match uploaded content`` (intermittent
    under parallel Tilt image publishes).
    """
    env = os.environ.copy()
    env["BUILDX_NO_DEFAULT_ATTESTATIONS"] = "1"
    cmd = [
        "docker",
        "buildx",
        "build",
        "--provenance=false",
        "--sbom=false",
        "--output",
        f"type=image,name={tag},push=true,oci-mediatypes=true",
        "-f",
        "-",
        ".",
    ]
    dockerfile = f"FROM {tag}\n"
    last: subprocess.CompletedProcess[str] | None = None
    for attempt in range(1, attempts + 1):
        last = subprocess.run(
            cmd,
            cwd=str(root),
            input=dockerfile,
            text=True,
            capture_output=True,
            env=env,
        )
        if last.returncode == 0:
            return last
        err = f"{last.stderr or ''}{last.stdout or ''}"
        if "digest did not match" in err.lower() and attempt < attempts:
            print(
                f"⚠️  Zot digest mismatch pushing {tag} (attempt {attempt}/{attempts}); retrying…",
                file=sys.stderr,
            )
            time.sleep(2 * attempt)
            continue
        return last
    assert last is not None
    return last


def _push_remote(root: Path, remote_tag: str) -> bool:
    """Push ``remote_tag`` to its registry. Prefer plain docker push, then skopeo/crane, then buildx OCI.

    BuildKit's docker driver + containerd image store often fails zot OCI pushes with
    ``provided digest did not match uploaded content`` / ``blob upload unknown``.
    Plain ``docker push`` (schema2) or skopeo usually works against the same zot.
    """
    legacy = subprocess.run(
        ["docker", "push", remote_tag], cwd=str(root), capture_output=True, text=True
    )
    if legacy.returncode == 0:
        print(f"✅ Docker image pushed to registry: {remote_tag}")
        return True
    legacy_err = f"{legacy.stderr or ''}{legacy.stdout or ''}".strip()
    if legacy_err:
        print(f"⚠️  docker push failed: {legacy_err.splitlines()[-1]}", file=sys.stderr)

    if subprocess.run(["which", "skopeo"], capture_output=True).returncode == 0:
        sk = subprocess.run(
            [
                "skopeo",
                "copy",
                "--dest-tls-verify=false",
                f"docker-daemon:{remote_tag}",
                f"docker://{remote_tag}",
            ],
            cwd=str(root),
            capture_output=True,
            text=True,
        )
        if sk.returncode == 0:
            print(f"✅ Docker image pushed via skopeo: {remote_tag}")
            return True
        sk_err = f"{sk.stderr or ''}{sk.stdout or ''}".strip()
        if sk_err:
            print(f"⚠️  skopeo copy failed: {sk_err.splitlines()[-1]}", file=sys.stderr)

    if subprocess.run(["which", "crane"], capture_output=True).returncode == 0:
        # crane push reads an OCI layout / tarball — use docker save.
        save = subprocess.run(
            ["docker", "save", remote_tag],
            cwd=str(root),
            capture_output=True,
        )
        if save.returncode == 0:
            crane = subprocess.run(
                ["crane", "push", "-", remote_tag],
                cwd=str(root),
                input=save.stdout,
                capture_output=True,
            )
            if crane.returncode == 0:
                print(f"✅ Docker image pushed via crane: {remote_tag}")
                return True
            crane_err = (crane.stderr or b"").decode("utf-8", errors="replace").strip()
            if crane_err:
                print(f"⚠️  crane push failed: {crane_err.splitlines()[-1]}", file=sys.stderr)

    print("⚠️  Trying buildx OCI push (often flakes on zot)…", file=sys.stderr)
    push = _push_oci_tag(root, remote_tag)
    if push.returncode == 0:
        print(f"✅ Docker image pushed to registry (OCI): {remote_tag}")
        return True
    err = f"{push.stderr or ''}{push.stdout or ''}".strip()
    if err:
        print(f"⚠️  OCI push failed: {err.splitlines()[-1]}", file=sys.stderr)
    return False


def _maybe_prune_dangling(prune_dangling_after: bool | None) -> None:
    """Optionally run ``docker image prune -f`` after a successful build."""
    from brrtrouter_tooling.docker import cleanup

    do = (
        prune_dangling_after
        if prune_dangling_after is not None
        else cleanup.env_prune_after_build()
    )
    if not do:
        return
    print(
        "🧹 Pruning dangling Docker images "
        "(set BRRTR_DOCKER_PRUNE_DANGLING_AFTER_BUILD=1 or pass --prune-dangling)…",
        file=sys.stderr,
    )
    cleanup.prune_dangling_images()


def _run_dev_sync_only(
    root: Path,
    image_name: str,
    base_image_name: str,
    kind_cluster_name: str,
    prune_dangling_after: bool | None,
) -> int:
    """Tag base as service image and push or kind load. Used when ``dev_sync_only`` is True."""
    owner = (
        os.environ.get("GHCR_OWNER") or os.environ.get("GITHUB_REPOSITORY_OWNER") or "microscaler"
    )
    base_image_local = f"{base_image_name}:latest"
    base_image_ghcr = f"ghcr.io/{owner}/{base_image_name}:latest"

    check_local = subprocess.run(
        ["docker", "images", "-q", base_image_local],
        capture_output=True,
        text=True,
        cwd=str(root),
    )
    check_ghcr = subprocess.run(
        ["docker", "images", "-q", base_image_ghcr],
        capture_output=True,
        text=True,
        cwd=str(root),
    )

    target_base = base_image_local
    if not (check_local.stdout and check_local.stdout.strip()):
        if check_ghcr.stdout and check_ghcr.stdout.strip():
            target_base = base_image_ghcr
        else:
            print(f"📦 Base image {base_image_local} or {base_image_ghcr} not found")
            print(f"   Attempting to pull from GHCR: {base_image_ghcr}")
            pull_result = subprocess.run(
                ["docker", "pull", base_image_ghcr],
                capture_output=True,
                text=True,
                cwd=str(root),
            )
            if pull_result.returncode == 0:
                target_base = base_image_ghcr
            else:
                print(f"   Pull failed, building locally as {base_image_local}...")
                from brrtrouter_tooling.docker.build_base import run as run_build_base

                if (
                    run_build_base(root, push=False, dry_run=False, base_image_name=base_image_name)
                    != 0
                ):
                    print("❌ Failed to build base image", file=sys.stderr)
                    return 1
                target_base = base_image_local

    tag_name = f"{image_name}:tilt"
    print(f"🚀 dev-sync-only mode: tagging {target_base} directly to {tag_name}")
    tr = subprocess.run(["docker", "tag", target_base, tag_name], cwd=str(root))
    if tr.returncode != 0:
        print("❌ Failed to alias base image", file=sys.stderr)
        return 1

    push = subprocess.run(
        ["docker", "push", tag_name], cwd=str(root), capture_output=True, text=True
    )
    if push.returncode == 0:
        print(f"✅ Docker image pushed to registry: {tag_name}")
    else:
        print("⚠️  Registry not available at localhost:5001; loading into Kind cluster...")
        kind = subprocess.run(
            ["kind", "load", "docker-image", tag_name, "--name", kind_cluster_name],
            cwd=str(root),
            capture_output=True,
            text=True,
        )
        if kind.returncode == 0:
            print(f"✅ Image loaded into Kind: {tag_name}")
        else:
            print(f"⚠️  Could not push or kind load; image tagged as: {tag_name}")
    print(f"✅ Docker image ready: {tag_name}")
    _maybe_prune_dangling(prune_dangling_after)
    return 0


def run(
    image_name: str,
    hash_path: Path,
    artifact_path: Path,
    project_root: Path,
    system: str | None = None,
    module: str | None = None,
    port: int | None = None,
    binary_name: str | None = None,
    dockerfile: Path | None = None,
    base_image_name: str = "brrtrouter-base",
    kind_cluster_name: str = "rerp",
    dev_sync_only: bool = False,
    no_cache: bool = False,
    prune_dangling_after: bool | None = None,
) -> int:
    """Build Docker image using template or static Dockerfile. Returns 0 on success, 1 on error."""
    root = project_root
    temp_dockerfile_path = None
    h = root / hash_path if not hash_path.is_absolute() else hash_path
    a = root / artifact_path if not artifact_path.is_absolute() else artifact_path

    if not h.exists():
        print(f"❌ Hash file not found: {h}", file=sys.stderr)
        print("   This indicates copy script has not completed yet", file=sys.stderr)
        return 1
    if not a.exists():
        print(f"❌ Artifact not found: {a}", file=sys.stderr)
        return 1

    if dev_sync_only:
        return _run_dev_sync_only(
            root, image_name, base_image_name, kind_cluster_name, prune_dangling_after
        )

    use_template = (
        system is not None and module is not None and port is not None and binary_name is not None
    )

    if use_template:
        template_path = root / "docker" / "microservices" / "Dockerfile.template"
        if not template_path.exists():
            internal_tpl = (
                Path(__file__).parent.parent
                / "templates"
                / "docker"
                / "microservices"
                / "Dockerfile.template"
            )
            if internal_tpl.exists():
                template_path = internal_tpl
            else:
                print(
                    f"❌ Template not found: {template_path} nor bundled template", file=sys.stderr
                )
                return 1

        template_content = template_path.read_text()
        entrypoint_literal = f"/app/{binary_name}"
        template_content = template_content.replace(
            '"/app/${BINARY_NAME}"', f'"{entrypoint_literal}"'
        )
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".Dockerfile", delete=False, dir=str(root)
        ) as tmp_fd:
            tmp_fd.write(template_content)
            dockerfile_path = Path(tmp_fd.name)
            temp_dockerfile_path = dockerfile_path

        build_args = [
            "--build-arg",
            f"SYSTEM={system}",
            "--build-arg",
            f"MODULE={module}",
            "--build-arg",
            f"PORT={port}",
            "--build-arg",
            f"BINARY_NAME={binary_name}",
            "--build-arg",
            f"BASE_IMAGE={base_image_name}:latest",
        ]
    else:
        if dockerfile is None:
            print(
                "❌ Either provide (system, module, port, binary_name) or dockerfile path",
                file=sys.stderr,
            )
            return 1
        d = root / dockerfile if not dockerfile.is_absolute() else dockerfile
        if not d.exists():
            print(f"❌ Dockerfile not found: {d}", file=sys.stderr)
            return 1
        dockerfile_path = d
        build_args = [
            "--build-arg",
            f"BASE_IMAGE={base_image_name}:latest",
        ]
        temp_dockerfile_path = None

    # Build to a LOCAL tag only. Tagging the build as registry:5000/... with the
    # containerd image store makes BuildKit "unpack to" the registry name and
    # subsequent OCI pushes to zot fail with digest / blob-upload errors.
    remote_tag = f"{image_name}:tilt"
    local_repo = image_name.rsplit("/", 1)[-1]
    local_tag = f"brrtr-local/{local_repo}:tilt"
    # Disable BuildKit provenance/SBOM attestations. Retagging + pushing an
    # attestation-bearing image to the LAN registry (distribution/zot) fails with
    # "provided digest did not match uploaded content".
    build_cmd = [
        "docker",
        "build",
        "-t",
        local_tag,
        "--rm",
        "--force-rm",
        "--provenance=false",
        "--sbom=false",
        "-f",
        str(dockerfile_path),
        *build_args,
        ".",
    ]
    if no_cache:
        build_cmd.insert(2, "--no-cache")
    try:
        build = subprocess.run(
            build_cmd,
            cwd=str(root),
        )

        if build.returncode != 0:
            print("❌ Docker build failed", file=sys.stderr)
            return 1

        tag_remote = subprocess.run(
            ["docker", "tag", local_tag, remote_tag], cwd=str(root), capture_output=True, text=True
        )
        if tag_remote.returncode != 0:
            print(f"❌ docker tag {local_tag} → {remote_tag} failed", file=sys.stderr)
            return 1

        pushed = _push_remote(root, remote_tag)
        if not pushed:
            # Kind/local workflow: load into the named cluster when registry push fails.
            is_lan_registry = "10.177." in image_name or image_name.startswith("192.168.")
            if is_lan_registry:
                print(
                    f"❌ Failed to push {remote_tag} to the LAN registry "
                    "(docker/skopeo/crane/buildx all failed).",
                    file=sys.stderr,
                )
                return 1
            print("⚠️  Registry push failed; loading into Kind cluster...", file=sys.stderr)
            kind = subprocess.run(
                ["kind", "load", "docker-image", remote_tag, "--name", kind_cluster_name],
                cwd=str(root),
                capture_output=True,
                text=True,
            )
            if kind.returncode == 0:
                print(f"✅ Image loaded into Kind: {remote_tag}")
            else:
                print(f"⚠️  Could not push or kind load; image tagged as: {remote_tag}")
                return 1
        print(f"✅ Docker image ready: {remote_tag}")
        _maybe_prune_dangling(prune_dangling_after)
        return 0
    finally:
        if temp_dockerfile_path is not None:
            temp_dockerfile_path.unlink(missing_ok=True)
