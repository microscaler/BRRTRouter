"""Build Docker image and push (or kind load)."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path


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
    base_image_name: str = "rerp-base",
    kind_cluster_name: str = "rerp",
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

    use_template = (
        system is not None and module is not None and port is not None and binary_name is not None
    )

    if use_template:
        template_path = root / "docker" / "microservices" / "Dockerfile.template"
        if not template_path.exists():
            print(f"❌ Template not found: {template_path}", file=sys.stderr)
            return 1

        owner = (
            os.environ.get("GHCR_OWNER")
            or os.environ.get("GITHUB_REPOSITORY_OWNER")
            or "microscaler"
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

        if not (check_local.stdout and check_local.stdout.strip()) and not (
            check_ghcr.stdout and check_ghcr.stdout.strip()
        ):
            print(f"📦 Base image {base_image_local} or {base_image_ghcr} not found")
            print(f"   Attempting to pull from GHCR: {base_image_ghcr}")
            pull_result = subprocess.run(
                ["docker", "pull", base_image_ghcr],
                capture_output=True,
                text=True,
                cwd=str(root),
            )
            if pull_result.returncode != 0:
                print(f"   Pull failed, building locally as {base_image_local}...")
                from brrtrouter_tooling.docker.build_base import run as run_build_base

                if (
                    run_build_base(root, push=False, dry_run=False, base_image_name=base_image_name)
                    != 0
                ):
                    print("❌ Failed to build base image", file=sys.stderr)
                    return 1
                tag_result = subprocess.run(
                    ["docker", "tag", base_image_local, base_image_ghcr],
                    capture_output=True,
                    text=True,
                    cwd=str(root),
                )
                if tag_result.returncode == 0:
                    print(f"   Tagged local image as {base_image_ghcr}")

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
        build_args = []
        temp_dockerfile_path = None

    tag = f"{image_name}:tilt"
    build_cmd = [
        "docker",
        "build",
        "-t",
        tag,
        "--rm",
        "--force-rm",
        "-f",
        str(dockerfile_path),
        *build_args,
        ".",
    ]
    try:
        build = subprocess.run(
            build_cmd,
            cwd=str(root),
        )

        if build.returncode != 0:
            print("❌ Docker build failed", file=sys.stderr)
            return 1

        push = subprocess.run(
            ["docker", "push", tag], cwd=str(root), capture_output=True, text=True
        )
        if push.returncode == 0:
            print(f"✅ Docker image pushed to registry: {tag}")
        else:
            print("⚠️  Registry not available at localhost:5001; loading into Kind cluster...")
            kind = subprocess.run(
                ["kind", "load", "docker-image", tag, "--name", kind_cluster_name],
                cwd=str(root),
                capture_output=True,
                text=True,
            )
            if kind.returncode == 0:
                print(f"✅ Image loaded into Kind: {tag}")
            else:
                print(f"⚠️  Could not push or kind load; image tagged as: {tag}")
        print(f"✅ Docker image ready: {tag}")
        return 0
    finally:
        if temp_dockerfile_path is not None:
            temp_dockerfile_path.unlink(missing_ok=True)
