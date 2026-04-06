"""BRRTRouter MCP (Model Context Protocol) server.

Exposes tools, resources, and prompts to help AI assistants:
- Build OpenAPI specs conformant to BRRTRouter
- Use brrtrouter-gen (code generator)
- Understand the generated (gen) and implementation (impl) directory layout
- Set up Backend-for-Frontend (BFF) services

Note: ``create_mcp_server`` and ``run_server`` are imported lazily so that
submodules (``tools``, ``resources``) can be used without the ``mcp`` package
installed.  The ``mcp`` extra is required only when actually running the server.
"""

from __future__ import annotations

__all__ = ["create_mcp_server", "run_server"]


def create_mcp_server():  # type: ignore[return]
    """Lazily import and return a configured FastMCP server instance."""
    from brrtrouter_tooling.mcp.server import create_mcp_server as _create

    return _create()


def run_server(*args, **kwargs):  # type: ignore[return]
    """Lazily import and run the MCP server."""
    from brrtrouter_tooling.mcp.server import run_server as _run

    return _run(*args, **kwargs)
