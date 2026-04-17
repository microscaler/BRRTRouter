"""Main CLI entry point for BRRTRouter tooling."""

import sys

from brrtrouter_tooling.cli import (
    bff,
    bootstrap_cmd,
    ci_cmd,
    dependabot,
    docker_cmd,
    gen_cmd,
    mcp_cmd,
    openapi_cmd,
    ports,
    pre_commit_cmd,
    release_cmd,
    tilt,
)
from brrtrouter_tooling.cli import (
    build as build_cli,
)


def main() -> None:
    """Main CLI entry point."""
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        print("Usage: brrtrouter <scope> <command> [args...]", file=sys.stderr)
        print("Scopes:", file=sys.stderr)
        print(
            "  client  - Commands for consumer repositories (gen, openapi, build, etc.)",
            file=sys.stderr,
        )
        print(
            "  local   - Commands for BRRTRouter repository maintenance (ci, dependabot, etc.)",
            file=sys.stderr,
        )
        sys.exit(0 if (len(sys.argv) >= 2 and sys.argv[1] in ("-h", "--help")) else 1)

    scope = sys.argv[1]

    if scope == "client":
        if len(sys.argv) < 3:
            print("Usage: brrtrouter client <command> [args...]", file=sys.stderr)
            print("Commands:", file=sys.stderr)
            print(
                "  gen generate|generate-stubs - Call brrtrouter-gen (gen crate or impl stubs)",
                file=sys.stderr,
            )
            print(
                "  openapi <subcommand>        - validate, fix-operation-id-casing, check-decimal-formats, etc.",
                file=sys.stderr,
            )
            print("  bootstrap microservice      - Bootstrap crate from OpenAPI", file=sys.stderr)
            print("  build <target>              - Host-aware build", file=sys.stderr)
            print(
                "  docker <cmd>...             - generate-dockerfile, build-image-simple, etc.",
                file=sys.stderr,
            )
            print("  bff generate|generate-system- Generate BFF OpenAPI specs", file=sys.stderr)
            print("  ports validate              - Scan and report port conflicts", file=sys.stderr)
            print(
                "  tilt <subcommand>           - Execute Tilt setup and lifecycle commands",
                file=sys.stderr,
            )
            print(
                "  mcp serve                   - Start the BRRTRouter MCP server (stdio or SSE)",
                file=sys.stderr,
            )
            sys.exit(1)

        sys.argv.pop(1)  # Strip scope to preserve downstream parsing logic
        command = sys.argv[1]

        if command == "gen":
            gen_cmd.run_gen_argv()
        elif command == "openapi":
            openapi_cmd.run_openapi_argv()
        elif command == "bootstrap":
            bootstrap_cmd.run_bootstrap_argv()
        elif command == "build":
            build_cli.run_build_argv()
        elif command == "docker":
            docker_cmd.run_docker_argv()
        elif command == "bff":
            if len(sys.argv) < 3:
                print("Usage: brrtrouter client bff <generate|generate-system>", file=sys.stderr)
                sys.exit(1)
            subcommand = sys.argv[2]
            if subcommand == "generate":
                bff.run_bff_generate()
            elif subcommand == "generate-system":
                bff.run_bff_generate_system_argv()
            else:
                print(f"Error: Unknown bff subcommand: {subcommand}", file=sys.stderr)
                sys.exit(1)
        elif command == "ports":
            ports.run_ports_argv()
        elif command == "tilt":
            tilt.run_tilt_argv()
        elif command == "mcp":
            mcp_cmd.run_mcp_argv()
        else:
            print(f"Error: Unknown client command: {command}", file=sys.stderr)
            sys.exit(1)

    elif scope == "local":
        if len(sys.argv) < 3:
            print("Usage: brrtrouter local <command> [args...]", file=sys.stderr)
            print("Commands:", file=sys.stderr)
            print(
                "  dependabot automerge        - Process and auto-merge Dependabot PRs",
                file=sys.stderr,
            )
            print(
                "  ci <subcommand>             - patch-brrtrouter, fix-cargo-paths, etc.",
                file=sys.stderr,
            )
            print(
                "  release bump|generate-notes - Bump Cargo version, generate release notes",
                file=sys.stderr,
            )
            print("  pre-commit workspace-fmt    - Run cargo fmt", file=sys.stderr)
            sys.exit(1)

        sys.argv.pop(1)  # Strip scope
        command = sys.argv[1]

        if command == "dependabot":
            if len(sys.argv) < 3:
                print("Usage: brrtrouter local dependabot automerge", file=sys.stderr)
                sys.exit(1)
            subcommand = sys.argv[2]
            if subcommand == "automerge":
                dependabot.automerge()
            else:
                print(f"Error: Unknown dependabot subcommand: {subcommand}", file=sys.stderr)
                sys.exit(1)
        elif command == "ci":
            ci_cmd.run_ci_argv()
        elif command == "release":
            release_cmd.run_release_argv()
        elif command == "pre-commit":
            pre_commit_cmd.run_pre_commit_argv()
        else:
            print(f"Error: Unknown local command: {command}", file=sys.stderr)
            sys.exit(1)

    else:
        print(f"Error: Unknown scope: {scope}. Must be 'client' or 'local'.", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
