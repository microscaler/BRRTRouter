"""`brrtrouter docker` subcommands: generate-dockerfile, copy-binary, build-base, build-image-simple, prune, copy-multiarch, build-multiarch, unpack-build-bins."""

import sys
from pathlib import Path

from brrtrouter_tooling.docker import cleanup as docker_cleanup
from brrtrouter_tooling.docker.build_base import run as run_build_base
from brrtrouter_tooling.docker.build_image_simple import run as run_build_image_simple
from brrtrouter_tooling.docker.build_multiarch import run as run_build_multiarch
from brrtrouter_tooling.docker.copy_binary import run as run_copy_binary
from brrtrouter_tooling.docker.copy_multiarch import run as run_copy_multiarch
from brrtrouter_tooling.docker.generate_dockerfile import run as run_generate_dockerfile
from brrtrouter_tooling.docker.unpack_build_bins import run as run_unpack_build_bins


def run_docker_argv(argv: list[str] | None = None) -> None:
    """Parse docker subcommand from argv and run. argv defaults to sys.argv[2:] when called from main."""
    if argv is None:
        argv = sys.argv[2:] if len(sys.argv) > 2 else []
    if not argv:
        print("brrtrouter docker: missing subcommand", file=sys.stderr)
        print(
            "  generate-dockerfile, copy-binary, build-base, build-image-simple, prune, copy-multiarch, build-multiarch, unpack-build-bins",
            file=sys.stderr,
        )
        sys.exit(1)
    cmd = argv[0]
    rest = argv[1:]
    project_root = Path.cwd()

    if cmd == "generate-dockerfile":
        if len(rest) < 2:
            print(
                "Usage: brrtrouter docker generate-dockerfile <system> <module> [--port N]",
                file=sys.stderr,
            )
            sys.exit(1)
        system, module = rest[0], rest[1]
        port = 8000
        for i, a in enumerate(rest):
            if a == "--port" and i + 1 < len(rest):
                port = int(rest[i + 1])
                break
        rc = run_generate_dockerfile(system, module, port=port, project_root=project_root)
        sys.exit(rc)

    if cmd == "unpack-build-bins":
        inp = Path(rest[0]) if rest else Path("tmp/buildBins")
        inp = (project_root / inp) if not inp.is_absolute() else inp
        rc = run_unpack_build_bins(inp, project_root)
        sys.exit(rc)

    if cmd == "copy-binary":
        if len(rest) < 3:
            print(
                "Usage: brrtrouter docker copy-binary <source> <dest> <binary_name>",
                file=sys.stderr,
            )
            sys.exit(1)
        rc = run_copy_binary(Path(rest[0]), Path(rest[1]), rest[2], project_root)
        sys.exit(rc)

    if cmd == "build-base":
        push = "--push" in rest
        dry_run = "--dry-run" in rest
        rc = run_build_base(project_root, push=push, dry_run=dry_run)
        sys.exit(rc)

    if cmd == "build-image-simple":
        if len(rest) < 3:
            print(
                "Usage: brrtrouter docker build-image-simple <image_name> <hash_path> <artifact_path> "
                "[--system S --module M --port N --binary-name B] [--no-cache] [--prune-dangling]",
                file=sys.stderr,
            )
            sys.exit(1)
        image_name, hash_path, artifact_path = rest[0], Path(rest[1]), Path(rest[2])
        system = module = port = binary_name = None
        dev_sync_only = False
        no_cache = "--no-cache" in rest
        prune_dangling_after = True if "--prune-dangling" in rest else None
        i = 3
        while i < len(rest):
            if rest[i] == "--system" and i + 1 < len(rest):
                system = rest[i + 1]
                i += 2
            elif rest[i] == "--module" and i + 1 < len(rest):
                module = rest[i + 1]
                i += 2
            elif rest[i] == "--port" and i + 1 < len(rest):
                port = int(rest[i + 1])
                i += 2
            elif rest[i] == "--binary-name" and i + 1 < len(rest):
                binary_name = rest[i + 1]
                i += 2
            elif rest[i] == "--dev-sync-only":
                dev_sync_only = True
                i += 1
            else:
                i += 1
        rc = run_build_image_simple(
            image_name,
            hash_path,
            artifact_path,
            project_root,
            system=system,
            module=module,
            port=port,
            binary_name=binary_name,
            dev_sync_only=dev_sync_only,
            no_cache=no_cache,
            prune_dangling_after=prune_dangling_after,
        )
        sys.exit(rc)

    if cmd == "prune":
        mode = rest[0] if rest else "dev"
        if mode in ("help", "-h", "--help"):
            print(
                "Usage: brrtrouter docker prune <dangling|containers|buildx|dev>",
                file=sys.stderr,
            )
            print(
                "  dangling   — docker image prune -f (dangling <none> images)",
                file=sys.stderr,
            )
            print("  containers — docker container prune -f (stopped containers)", file=sys.stderr)
            print("  buildx     — docker buildx prune -f (BuildKit cache)", file=sys.stderr)
            print(
                "  dev        — all of the above in order (default)",
                file=sys.stderr,
            )
            sys.exit(0)
        if mode == "dangling":
            sys.exit(docker_cleanup.prune_dangling_images())
        if mode == "containers":
            sys.exit(docker_cleanup.prune_stopped_containers())
        if mode == "buildx":
            sys.exit(docker_cleanup.prune_buildx_cache())
        if mode == "dev":
            sys.exit(docker_cleanup.prune_dev_sweep())
        print(f"Unknown prune mode: {mode}", file=sys.stderr)
        sys.exit(1)

    if cmd == "copy-multiarch":
        if len(rest) < 2:
            print(
                "Usage: brrtrouter docker copy-multiarch <system> <module> [arch]", file=sys.stderr
            )
            sys.exit(1)
        system, module = rest[0], rest[1]
        arch = rest[2] if len(rest) > 2 else "all"
        rc = run_copy_multiarch(system, module, arch, project_root)
        sys.exit(rc)

    if cmd == "build-multiarch":
        if len(rest) < 3:
            print(
                "Usage: brrtrouter docker build-multiarch <system> <module> <image_name> [--tag T] [--push] [--build-cmd 'cmd ...']",
                file=sys.stderr,
            )
            sys.exit(1)
        system, module, image_name = rest[0], rest[1], rest[2]
        tag = "latest"
        push = False
        build_cmd = None
        i = 3
        while i < len(rest):
            if rest[i] == "--tag" and i + 1 < len(rest):
                tag = rest[i + 1]
                i += 2
            elif rest[i] == "--push":
                push = True
                i += 1
            elif rest[i] == "--build-cmd" and i + 1 < len(rest):
                build_cmd = rest[i + 1].split()
                i += 2
            else:
                i += 1
        if not build_cmd:
            build_cmd = ["brrtrouter", "build", f"{system}_{module}", "all"]
        rc = run_build_multiarch(
            system,
            module,
            image_name,
            tag,
            push,
            project_root,
            build_cmd=build_cmd,
        )
        sys.exit(rc)

    print(f"Unknown docker subcommand: {cmd}", file=sys.stderr)
    sys.exit(1)
