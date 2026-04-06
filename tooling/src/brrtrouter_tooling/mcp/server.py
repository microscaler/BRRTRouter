"""BRRTRouter MCP server: assembles tools, resources, and prompts.

Usage (programmatic):
    from brrtrouter_tooling.mcp import create_mcp_server, run_server
    server = create_mcp_server()
    server.run()

Usage (CLI):
    brrtrouter mcp serve [--transport stdio|sse] [--host 127.0.0.1] [--port 8765]
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from mcp.server.fastmcp import FastMCP

from brrtrouter_tooling.mcp import prompts as _prompts
from brrtrouter_tooling.mcp import resources as _resources
from brrtrouter_tooling.mcp import tools as _tools

if TYPE_CHECKING:
    pass

_SERVER_NAME = "BRRTRouter"
_SERVER_DESCRIPTION = (
    "BRRTRouter MCP server — helps AI assistants build OpenAPI specs conformant to "
    "BRRTRouter, use brrtrouter-gen, understand the gen/impl directory layout, and "
    "set up Backend-for-Frontend (BFF) services."
)


def create_mcp_server() -> FastMCP:
    """Create and configure the BRRTRouter FastMCP server instance.

    Registers all tools, resources, and prompts and returns the server.
    Use ``server.run()`` or ``run_server()`` to start it.

    Returns:
        Configured :class:`FastMCP` instance.
    """
    mcp = FastMCP(_SERVER_NAME, instructions=_SERVER_DESCRIPTION)

    # ------------------------------------------------------------------
    # Tools
    # ------------------------------------------------------------------

    @mcp.tool()
    def lint_spec(spec_content: str, project_root: str | None = None) -> str:
        """Lint an OpenAPI spec string for BRRTRouter conformance.

        Checks openapi version, snake_case operationIds, unresolved $refs,
        and (optionally) runs brrtrouter-gen lint if project_root is provided.

        Args:
            spec_content: Full OpenAPI YAML content as a string.
            project_root: Optional path to a BRRTRouter workspace for brrtrouter-gen lint.
        """
        return _tools.lint_spec(spec_content, project_root)

    @mcp.tool()
    def check_spec_conformance(spec_content: str) -> str:
        """Check an OpenAPI spec for BRRTRouter-specific conformance rules.

        Performs a comprehensive review covering version, operationIds,
        error response format (RFC 7807), $ref resolution, number formats,
        and SSE extension usage.

        Args:
            spec_content: Full OpenAPI YAML content as a string.
        """
        return _tools.check_spec_conformance(spec_content)

    @mcp.tool()
    def validate_openapi_dir(openapi_dir: str) -> str:
        """Validate all openapi.yaml files found under a directory for YAML validity.

        Args:
            openapi_dir: Path to directory tree to scan for openapi.yaml files.
        """
        return _tools.validate_openapi_dir(openapi_dir)

    @mcp.tool()
    def fix_operation_ids(openapi_dir: str, dry_run: bool = True) -> str:
        """Convert all operationIds in a directory tree to snake_case.

        Args:
            openapi_dir: Root directory to scan for openapi.yaml files.
            dry_run: When True (default), report changes without writing files.
        """
        return _tools.fix_operation_ids(openapi_dir, dry_run=dry_run)

    @mcp.tool()
    def list_spec_operations(spec_path: str) -> str:
        """List all operations (METHOD /path operationId) in an OpenAPI spec file.

        Args:
            spec_path: Path to an OpenAPI YAML spec file.
        """
        return _tools.list_spec_operations(spec_path)

    @mcp.tool()
    def generate_project(
        spec_path: str,
        output_dir: str,
        project_root: str,
        brrtrouter_path: str | None = None,
        deps_config_path: str | None = None,
        package_name: str | None = None,
    ) -> str:
        """Generate a complete Rust gen crate from an OpenAPI spec via brrtrouter-gen.

        Args:
            spec_path: Path to the OpenAPI 3.1.0 YAML spec.
            output_dir: Directory where the generated crate will be written.
            project_root: Rust workspace root (cwd for cargo run).
            brrtrouter_path: Optional path to BRRTRouter checkout (defaults to ../BRRTRouter).
            deps_config_path: Optional path to brrtrouter-dependencies.toml.
            package_name: Optional Cargo package name for the generated crate.
        """
        return _tools.generate_project(
            spec_path=spec_path,
            output_dir=output_dir,
            project_root=project_root,
            brrtrouter_path=brrtrouter_path,
            deps_config_path=deps_config_path,
            package_name=package_name,
        )

    @mcp.tool()
    def generate_stubs(
        spec_path: str,
        impl_dir: str,
        component_name: str,
        project_root: str,
        brrtrouter_path: str | None = None,
        force: bool = False,
        sync: bool = False,
    ) -> str:
        """Generate implementation stub files (impl crate) for a BRRTRouter service.

        Files with the BRRTROUTER_USER_OWNED sentinel are not overwritten.
        Use sync=True to update only handler signatures without touching bodies.

        Args:
            spec_path: Path to the OpenAPI 3.1.0 YAML spec.
            impl_dir: Directory where impl stubs will be written.
            component_name: Name of the gen crate (for import paths in stubs).
            project_root: Rust workspace root (cwd for cargo run).
            brrtrouter_path: Optional path to BRRTRouter checkout.
            force: When True, overwrite existing stub files.
            sync: When True, only patch stub signatures (preserves user body).
        """
        return _tools.generate_stubs(
            spec_path=spec_path,
            impl_dir=impl_dir,
            component_name=component_name,
            project_root=project_root,
            brrtrouter_path=brrtrouter_path,
            force=force,
            sync=sync,
        )

    @mcp.tool()
    def generate_bff(
        suite_config_path: str,
        output_path: str | None = None,
        base_dir: str | None = None,
        validate: bool = True,
    ) -> str:
        """Generate a merged BFF OpenAPI spec from a suite config YAML.

        Merges multiple downstream service specs into a single BFF spec with
        path prefixing, schema prefixing (PascalCase service name), and
        proxy routing extensions (x-brrtrouter-downstream-path, x-service).

        Args:
            suite_config_path: Path to bff-suite-config.yaml.
            output_path: Optional override for the output spec path.
            base_dir: Optional base directory for resolving paths in the config.
            validate: When True, validate the generated spec after writing.
        """
        return _tools.generate_bff(
            suite_config_path=suite_config_path,
            output_path=output_path,
            base_dir=base_dir,
            validate=validate,
        )

    @mcp.tool()
    def inspect_generated_dir(gen_dir: str) -> str:
        """Inspect a generated (gen crate) directory and summarise its contents.

        Lists handler and controller files, package name, and config presence.

        Args:
            gen_dir: Path to the generated crate directory.
        """
        return _tools.inspect_generated_dir(gen_dir)

    @mcp.tool()
    def inspect_impl_dir(impl_dir: str) -> str:
        """Inspect an implementation (impl crate) directory.

        Shows which handler files are user-owned (BRRTROUTER_USER_OWNED sentinel)
        versus unmodified stubs.

        Args:
            impl_dir: Path to the impl crate directory.
        """
        return _tools.inspect_impl_dir(impl_dir)

    # ------------------------------------------------------------------
    # Resources
    # ------------------------------------------------------------------

    @mcp.resource("brrtrouter://guide/openapi-spec")
    def openapi_spec_guide() -> str:
        """Guide for writing BRRTRouter-conformant OpenAPI 3.1.0 specs."""
        return _resources.get_openapi_spec_guide()

    @mcp.resource("brrtrouter://guide/code-generation")
    def code_generation_guide() -> str:
        """Guide for brrtrouter-gen: gen crate, impl crate, and regeneration workflow."""
        return _resources.get_code_generation_guide()

    @mcp.resource("brrtrouter://guide/bff-pattern")
    def bff_pattern_guide() -> str:
        """Guide for setting up a Backend-for-Frontend (BFF) service with BRRTRouter."""
        return _resources.get_bff_pattern_guide()

    @mcp.resource("brrtrouter://reference/extensions")
    def extensions_reference() -> str:
        """Reference for all BRRTRouter-specific OpenAPI extensions (x-sse, x-cors, etc.)."""
        return _resources.get_extensions_reference()

    @mcp.resource("brrtrouter://examples/openapi-spec")
    def example_openapi_spec() -> str:
        """Minimal conformant OpenAPI 3.1.0 example spec for BRRTRouter."""
        return _resources.get_example_openapi_yaml()

    # ------------------------------------------------------------------
    # Prompts
    # ------------------------------------------------------------------

    @mcp.prompt()
    def write_openapi_spec(service_name: str, description: str) -> str:
        """Prime the assistant to write a BRRTRouter-conformant OpenAPI spec.

        Args:
            service_name: Name of the service (used as the spec title).
            description: Brief description of what the service does.
        """
        result = _prompts.write_openapi_spec_prompt(service_name, description)
        return result.messages[0].content.text  # type: ignore[union-attr]

    @mcp.prompt()
    def setup_bff(system_name: str, services: str) -> str:
        """Prime the assistant to create a BFF suite config and OpenAPI spec.

        Args:
            system_name: Name of the system/BFF.
            services: Comma-separated list of downstream service names.
        """
        svc_list = [s.strip() for s in services.split(",") if s.strip()]
        result = _prompts.setup_bff_prompt(system_name, svc_list)
        return result.messages[0].content.text  # type: ignore[union-attr]

    @mcp.prompt()
    def implement_handler(operation_id: str, request_type: str, response_type: str) -> str:
        """Prime the assistant to implement a BRRTRouter handler stub.

        Args:
            operation_id: The operationId (snake_case) of the handler.
            request_type: The Rust request struct type name.
            response_type: The Rust response struct type name.
        """
        result = _prompts.implement_handler_prompt(operation_id, request_type, response_type)
        return result.messages[0].content.text  # type: ignore[union-attr]

    @mcp.prompt()
    def review_spec(spec_content: str) -> str:
        """Prime the assistant to review and improve an OpenAPI spec.

        Args:
            spec_content: The OpenAPI YAML content to review.
        """
        result = _prompts.review_spec_prompt(spec_content)
        return result.messages[0].content.text  # type: ignore[union-attr]

    return mcp


def run_server(transport: str = "stdio", host: str = "127.0.0.1", port: int = 8765) -> None:
    """Create and run the BRRTRouter MCP server.

    Args:
        transport: Transport protocol — ``"stdio"`` (default, for Claude Desktop /
            CLI use) or ``"sse"`` (HTTP Server-Sent Events, for web clients).
        host: Bind host for SSE transport (default ``"127.0.0.1"``).
        port: Bind port for SSE transport (default ``8765``).
    """
    server = create_mcp_server()
    if transport == "sse":
        server.run(transport="sse", host=host, port=port)
    else:
        server.run(transport="stdio")
