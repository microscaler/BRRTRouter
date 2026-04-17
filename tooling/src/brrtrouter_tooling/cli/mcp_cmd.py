"""CLI for BRRTRouter MCP server: brrtrouter mcp serve."""

from __future__ import annotations

import sys


def run_mcp_argv() -> None:
    """Dispatch brrtrouter mcp <subcommand>."""
    args = sys.argv[2:]
    if not args or args[0] in ("-h", "--help"):
        _print_usage()
        sys.exit(0)

    sub = args[0].lower()
    if sub == "serve":
        _run_serve(args[1:])
    else:
        print(f"Error: Unknown mcp subcommand: {sub}", file=sys.stderr)
        _print_usage()
        sys.exit(1)


def _print_usage() -> None:
    print(
        "Usage: brrtrouter mcp serve [--transport stdio|sse] [--host HOST] [--port PORT]\n"
        "\n"
        "Subcommands:\n"
        "  serve   Start the BRRTRouter MCP server\n"
        "\n"
        "Options for serve:",
        file=sys.stderr,
    )
    print("  --transport stdio|sse   Transport protocol (default: stdio)", file=sys.stderr)
    print(
        "  --host HOST             Bind host for SSE transport (default: 127.0.0.1)",
        file=sys.stderr,
    )
    print("  --port PORT             Bind port for SSE transport (default: 8765)", file=sys.stderr)


def _run_serve(args: list[str]) -> None:
    transport = "stdio"
    host = "127.0.0.1"
    port = 8765

    i = 0
    while i < len(args):
        if args[i] == "--transport" and i + 1 < len(args):
            transport = args[i + 1]
            i += 2
        elif args[i] == "--host" and i + 1 < len(args):
            host = args[i + 1]
            i += 2
        elif args[i] == "--port" and i + 1 < len(args):
            try:
                port = int(args[i + 1])
            except ValueError:
                print(f"Error: --port must be an integer, got {args[i + 1]!r}", file=sys.stderr)
                sys.exit(1)
            if not (1 <= port <= 65535):
                print(
                    f"Error: --port must be between 1 and 65535, got {port}",
                    file=sys.stderr,
                )
                sys.exit(1)
            i += 2
        elif args[i] in ("-h", "--help"):
            _print_usage()
            sys.exit(0)
        else:
            print(f"Error: Unknown argument: {args[i]}", file=sys.stderr)
            _print_usage()
            sys.exit(1)

    if transport not in ("stdio", "sse"):
        print(f"Error: --transport must be 'stdio' or 'sse', got {transport!r}", file=sys.stderr)
        sys.exit(1)

    try:
        from brrtrouter_tooling.mcp import run_server
    except ImportError as e:
        print(
            f"Error: MCP server requires the 'mcp' package. "
            f"Install with: pip install 'brrtrouter-tooling[mcp]'\n{e}",
            file=sys.stderr,
        )
        sys.exit(1)

    if transport == "sse":
        print(f"Starting BRRTRouter MCP server (SSE) on {host}:{port} ...")
    else:
        print("Starting BRRTRouter MCP server (stdio) ...", file=sys.stderr)

    try:
        run_server(transport=transport, host=host, port=port)
    except (OSError, RuntimeError, ValueError) as e:
        print(f"Error: MCP server failed: {e}", file=sys.stderr)
        sys.exit(1)
