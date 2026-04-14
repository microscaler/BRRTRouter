"""`hauliage docker` subcommands: delegate to brrtrouter_tooling.docker with Hauliage params."""

import sys
from pathlib import Path

from brrtrouter_tooling.workspace.docker.copy_artifacts import run as run_copy_artifacts
from brrtrouter_tooling.workspace.docker.copy_artifacts import validate_build_artifacts
from brrtrouter_tooling.workspace.docker.render_dockerfile import (
    render_dockerfile_to_path,
    render_dockerfile_to_temp,
)
from brrtrouter_tooling.workspace.env_paths import venv_bin

# Hauliage-specific docker params (image names, kind cluster, paths)
Hauliage_BASE_IMAGE = "hauliage-base"
Hauliage_BASE_IMAGE_LOCAL = "hauliage/base"
Hauliage_DOCKER_DIR = "docker"
Hauliage_KIND_CLUSTER = "hauliage"
Hauliage_ZIP_PREFIX = "hauliage-binaries-"
Hauliage_BINARY_PATTERN = "hauliage_{system}_{module}_impl"


def run_docker(args, project_root: Path) -> None:
    if args.docker_cmd == "render-dockerfile":
        template_path = Path(args.template)
        if not template_path.is_absolute():
            template_path = project_root / template_path
        output_path = Path(args.output)
        render_dockerfile_to_path(
            project_root,
            template_path,
            args.service,
            output_path=output_path,
            system=getattr(args, "system", None),
        )
        sys.exit(0)
    if args.docker_cmd == "generate-dockerfile":
        from brrtrouter_tooling.docker.generate_dockerfile import run as run_generate_dockerfile

        rc = run_generate_dockerfile(
            args.system,
            args.module,
            port=getattr(args, "port", 8000),
            project_root=project_root,
            binary_name_pattern=Hauliage_BINARY_PATTERN,
        )
        sys.exit(rc)
    if args.docker_cmd == "unpack-build-bins":
        from brrtrouter_tooling.docker.unpack_build_bins import run as run_unpack_build_bins

        inp = Path(args.input_dir)
        inp = (project_root / inp) if not inp.is_absolute() else inp
        rc = run_unpack_build_bins(inp, project_root, zip_prefix=Hauliage_ZIP_PREFIX)
        sys.exit(rc)
    if args.docker_cmd == "validate-build-artifacts":
        rc = validate_build_artifacts(project_root, suite=getattr(args, "suite", None))
        sys.exit(rc)
    if args.docker_cmd == "copy-artifacts":
        rc = run_copy_artifacts(args.arch, project_root, suite=getattr(args, "suite", None))
        sys.exit(rc)
    if args.docker_cmd == "build-base":
        from brrtrouter_tooling.docker.build_base import run as run_build_base

        rc = run_build_base(
            project_root,
            push=args.push,
            dry_run=args.dry_run,
            base_image_name=Hauliage_BASE_IMAGE,
            docker_dir=Hauliage_DOCKER_DIR,
            tag_kind_registry=False,
            tag_dockerhub=False,
        )
        sys.exit(rc)
    if args.docker_cmd == "copy-binary":
        from brrtrouter_tooling.docker.copy_binary import run as run_copy_binary

        rc = run_copy_binary(
            Path(args.source),
            Path(args.dest),
            args.binary_name,
            project_root,
        )
        sys.exit(rc)
    if args.docker_cmd == "build-image-simple":
        dockerfile_path = Path(args.dockerfile)
        if not dockerfile_path.is_absolute():
            dockerfile_path = project_root / dockerfile_path
        service = getattr(args, "service", None)
        if service:
            # Render Dockerfile.template on the fly; no per-service Dockerfile needed
            from brrtrouter_tooling.docker.build_image_simple import run as run_build_image_simple

            temp_dockerfile = render_dockerfile_to_temp(project_root, dockerfile_path, service)
            try:
                rc = run_build_image_simple(
                    args.image_name,
                    Path(args.hash_path),
                    Path(args.artifact_path),
                    project_root,
                    dockerfile=temp_dockerfile,
                    kind_cluster_name=Hauliage_KIND_CLUSTER,
                    base_image_name=Hauliage_BASE_IMAGE,
                    dev_sync_only=getattr(args, "dev_sync_only", False),
                    no_cache=getattr(args, "no_cache", False),
                    prune_dangling_after=(True if getattr(args, "prune_dangling", False) else None),
                )
            finally:
                temp_dockerfile.unlink(missing_ok=True)
        else:
            from brrtrouter_tooling.docker.build_image_simple import run as run_build_image_simple

            rc = run_build_image_simple(
                args.image_name,
                Path(args.hash_path),
                Path(args.artifact_path),
                project_root,
                dockerfile=dockerfile_path,
                kind_cluster_name=Hauliage_KIND_CLUSTER,
                base_image_name=Hauliage_BASE_IMAGE,
                dev_sync_only=getattr(args, "dev_sync_only", False),
                no_cache=getattr(args, "no_cache", False),
                prune_dangling_after=(True if getattr(args, "prune_dangling", False) else None),
            )
        sys.exit(rc)
    if args.docker_cmd == "prune":
        from brrtrouter_tooling.docker import cleanup

        t = getattr(args, "prune_target", "dev")
        if t == "dangling":
            sys.exit(cleanup.prune_dangling_images())
        if t == "containers":
            sys.exit(cleanup.prune_stopped_containers())
        if t == "buildx":
            sys.exit(cleanup.prune_buildx_cache())
        if t == "dev":
            sys.exit(cleanup.prune_dev_sweep())
        print(f"Unknown prune target: {t}", file=sys.stderr)
        sys.exit(1)
    if args.docker_cmd == "copy-multiarch":
        from brrtrouter_tooling.docker.copy_multiarch import run as run_copy_multiarch

        rc = run_copy_multiarch(
            args.system,
            args.module,
            getattr(args, "arch", "all"),
            project_root,
        )
        sys.exit(rc)
    if args.docker_cmd == "build-multiarch":
        from brrtrouter_tooling.docker.build_multiarch import run as run_build_multiarch

        build_cmd = [
            venv_bin("hauliage"),
            "build",
            f"{args.system}_{args.module}",
            "all",
        ]
        rc = run_build_multiarch(
            args.system,
            args.module,
            args.image_name,
            getattr(args, "tag", "latest"),
            getattr(args, "push", False),
            project_root,
            build_cmd=build_cmd,
            base_image_name=Hauliage_BASE_IMAGE,
            docker_dir=Hauliage_DOCKER_DIR,
            base_image_local=Hauliage_BASE_IMAGE_LOCAL,
        )
        sys.exit(rc)
    sys.exit(1)
