"""BRRTRouter MCP (Model Context Protocol) server.

Exposes tools, resources, and prompts to help AI assistants:
- Build OpenAPI specs conformant to BRRTRouter
- Use brrtrouter-gen (code generator)
- Understand the generated (gen) and implementation (impl) directory layout
- Set up Backend-for-Frontend (BFF) services
"""

from brrtrouter_tooling.mcp.server import create_mcp_server, run_server

__all__ = ["create_mcp_server", "run_server"]
