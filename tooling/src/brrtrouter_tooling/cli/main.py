"""Main CLI entry point for BRRTRouter tooling."""

import sys

from brrtrouter_tooling.cli import (
    bff,
    dependabot,
    docker_cmd,
    openapi_cmd,
    ports,
    release_cmd,
)
from brrtrouter_tooling.cli import (
    build as build_cli,
)


def main() -> None:
    """Main CLI entry point."""
    if len(sys.argv) < 2:
        print("Usage: brrtrouter <command> [args...]", file=sys.stderr)
        print("Commands:", file=sys.stderr)
        print("  dependabot automerge  - Process and auto-merge Dependabot PRs", file=sys.stderr)
        print(
            "  bff generate          - Generate BFF OpenAPI spec from suite config",
            file=sys.stderr,
        )
        print(
            "  bff generate-system   - Generate system BFF from openapi/{system}/{svc}/openapi.yaml",
            file=sys.stderr,
        )
        print(
            "  ports validate        - Scan registry, helm, kind, Tiltfile; report port conflicts",
            file=sys.stderr,
        )
        print(
            "  openapi <subcommand>  - validate, fix-operation-id-casing, check-decimal-formats, fix-impl-controllers",
            file=sys.stderr,
        )
        print(
            "  build <target> [arch]  - Host-aware cargo/cross/zigbuild (workspace or system_module)",
            file=sys.stderr,
        )
        print(
            "  docker <cmd> ...       - generate-dockerfile, copy-binary, build-base, build-image-simple, copy-multiarch, build-multiarch, unpack-build-bins",
            file=sys.stderr,
        )
        print(
            "  release bump|generate-notes - Bump Cargo version; generate release notes (OpenAI/Anthropic)",
            file=sys.stderr,
        )
        sys.exit(1)

    command = sys.argv[1]

    if command == "dependabot":
        if len(sys.argv) < 3:
            print("Usage: brrtrouter dependabot <subcommand>", file=sys.stderr)
            print("Subcommands:", file=sys.stderr)
            print("  automerge  - Process and auto-merge Dependabot PRs", file=sys.stderr)
            sys.exit(1)

        subcommand = sys.argv[2]
        if subcommand == "automerge":
            dependabot.automerge()
        else:
            print(f"Error: Unknown dependabot subcommand: {subcommand}", file=sys.stderr)
            sys.exit(1)
    elif command == "bff":
        if len(sys.argv) < 3:
            print("Usage: brrtrouter bff <subcommand>", file=sys.stderr)
            print("Subcommands:", file=sys.stderr)
            print(
                "  generate         - Generate BFF OpenAPI spec from suite config", file=sys.stderr
            )
            print(
                "  generate-system  - Generate system BFF from openapi/{system}/{svc}/openapi.yaml",
                file=sys.stderr,
            )
            sys.exit(1)
        subcommand = sys.argv[2]
        if subcommand == "generate":
            bff.run_bff_generate()
        elif subcommand == "generate-system":
            bff.run_bff_generate_system_argv()
        else:
            print(f"Error: Unknown bff subcommand: {subcommand}", file=sys.stderr)
            sys.exit(1)
    elif command == "openapi":
        openapi_cmd.run_openapi_argv()
    elif command == "ports":
        if len(sys.argv) < 3:
            print("Usage: brrtrouter ports <subcommand>", file=sys.stderr)
            print("Subcommands:", file=sys.stderr)
            print(
                "  validate  - Scan registry, helm, kind, Tiltfile; report port conflicts",
                file=sys.stderr,
            )
            sys.exit(1)
        subcommand = sys.argv[2]
        if subcommand == "validate":
            ports.run_ports_validate_argv()
        else:
            print(f"Error: Unknown ports subcommand: {subcommand}", file=sys.stderr)
            sys.exit(1)
    elif command == "build":
        build_cli.run_build_argv()
    elif command == "docker":
        docker_cmd.run_docker_argv()
    elif command == "release":
        release_cmd.run_release_argv()
    else:
        print(f"Error: Unknown command: {command}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
