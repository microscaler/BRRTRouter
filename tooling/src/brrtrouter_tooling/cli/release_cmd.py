"""`brrtrouter release` subcommands: bump, generate-notes."""

import sys
from pathlib import Path

from brrtrouter_tooling.release.bump import run as run_bump
from brrtrouter_tooling.release.notes import run as run_notes


def run_release_argv(argv: list[str] | None = None) -> None:
    """Parse release subcommand from argv and run. argv defaults to sys.argv[2:] when called from main."""
    if argv is None:
        argv = sys.argv[2:] if len(sys.argv) > 2 else []
    if not argv:
        print("brrtrouter release: missing subcommand (bump, generate-notes)", file=sys.stderr)
        sys.exit(1)
    cmd = argv[0]
    rest = argv[1:]
    project_root = Path.cwd()

    if cmd == "bump":
        bump = rest[0] if rest else "patch"
        rc = run_bump(project_root, bump)
        sys.exit(rc)

    if cmd == "generate-notes":
        version = None
        output = None
        template = None
        since_tag = None
        provider = None
        model = None
        i = 0
        while i < len(rest):
            if rest[i] in ("--version", "-v") and i + 1 < len(rest):
                version = rest[i + 1]
                i += 2
            elif rest[i] in ("--output", "-o") and i + 1 < len(rest):
                output = Path(rest[i + 1])
                i += 2
            elif rest[i] in ("--template", "-t") and i + 1 < len(rest):
                template = Path(rest[i + 1])
                i += 2
            elif rest[i] == "--since-tag" and i + 1 < len(rest):
                since_tag = rest[i + 1]
                i += 2
            elif rest[i] == "--provider" and i + 1 < len(rest):
                provider = rest[i + 1]
                i += 2
            elif rest[i] == "--model" and i + 1 < len(rest):
                model = rest[i + 1]
                i += 2
            else:
                i += 1
        if not version:
            print("brrtrouter release generate-notes: --version is required", file=sys.stderr)
            sys.exit(1)
        rc = run_notes(
            project_root,
            version,
            since_tag=since_tag,
            template_path=template,
            output_path=output,
            model=model,
            provider=provider,
        )
        sys.exit(rc)

    print("brrtrouter release: use subcommand 'bump' or 'generate-notes'", file=sys.stderr)
    sys.exit(1)
