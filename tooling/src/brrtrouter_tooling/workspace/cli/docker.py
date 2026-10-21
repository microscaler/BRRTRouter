"""`<cli> docker` subcommands: delegate to brrtrouter_tooling.docker with the project profile's names."""

import sys
from pathlib import Path

from brrtrouter_tooling.workspace.docker.copy_artifacts import run as run_copy_artifacts
from brrtrouter_tooling.workspace.docker.copy_artifacts import validate_build_artifacts
from brrtrouter_tooling.workspace.docker.render_dockerfile import (
    render_dockerfile_to_path,
    render_dockerfile_to_temp,
)
from brrtrouter_tooling.workspace.env_paths import venv_bin
from brrtrouter_tooling.workspace.profile import ProjectProfile, load_profile

DOCKER_DIR = "docker"


def _binary_pattern(profile: ProjectProfile) -> str:
    """The profile's binary pattern with the prefix filled in ({system} and {module} stay for the generator)."""
    return profile.binary_pattern.replace("{prefix}", profile.package_prefix)


def _base_image_local(profile: ProjectProfile) -> str:
    """Local tag of the per-arch base image: ``<name>/base``."""
    return f"{profile.name}/base"


def run_docker(args, project_root: Path) -> None:
    profile = load_profile(project_root)
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
            binary_name_pattern=_binary_pattern(profile),
        )
        sys.exit(rc)
    if args.docker_cmd == "unpack-build-bins":
        from brrtrouter_tooling.docker.unpack_build_bins import run as run_unpack_build_bins

        inp = Path(args.input_dir)
        inp = (project_root / inp) if not inp.is_absolute() else inp
        rc = run_unpack_build_bins(inp, project_root, zip_prefix=profile.zip_prefix)
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
            base_image_name=profile.base_image,
            docker_dir=DOCKER_DIR,
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
                    kind_cluster_name=profile.kind_cluster,
                    base_image_name=profile.base_image,
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
                kind_cluster_name=profile.kind_cluster,
                base_image_name=profile.base_image,
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
            venv_bin(profile.cli),
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
            base_image_name=profile.base_image,
            docker_dir=DOCKER_DIR,
            base_image_local=_base_image_local(profile),
        )
        sys.exit(rc)
    sys.exit(1)
