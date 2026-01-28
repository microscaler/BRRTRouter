"""Main CLI entry point for BRRTRouter tooling."""

import sys
from typing import Optional

from brrtrouter_tooling.cli import dependabot


def main() -> None:
    """Main CLI entry point."""
    if len(sys.argv) < 2:
        print("Usage: brrtrouter <command> [args...]", file=sys.stderr)
        print("Commands:", file=sys.stderr)
        print("  dependabot automerge  - Process and auto-merge Dependabot PRs", file=sys.stderr)
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
    else:
        print(f"Error: Unknown command: {command}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
