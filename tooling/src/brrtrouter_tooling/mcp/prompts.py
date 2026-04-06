"""MCP prompt templates for BRRTRouter.

Each function returns a list of message dicts (role + content) that prime an
AI assistant for a specific BRRTRouter task.  Prompts are registered on the
MCP server and surfaced to clients that support the prompts capability.
"""

from __future__ import annotations

from mcp.types import GetPromptResult, PromptMessage, TextContent


def write_openapi_spec_prompt(service_name: str, description: str) -> GetPromptResult:
    """Return a prompt that instructs the assistant to write a BRRTRouter-conformant spec.

    Args:
        service_name: Name of the service (used as the spec title).
        description: Brief description of what the service does.
    """
    return GetPromptResult(
        description=f"Write a BRRTRouter-conformant OpenAPI 3.1.0 spec for '{service_name}'",
        messages=[
            PromptMessage(
                role="user",
                content=TextContent(
                    type="text",
                    text=(
                        f"Write a complete OpenAPI 3.1.0 spec for a service called '{service_name}'.\n"
                        f"Description: {description}\n\n"
                        "Rules to follow:\n"
                        "- openapi: 3.1.0 (required)\n"
                        "- Every operationId must be unique and snake_case (e.g. list_users, get_user_by_id)\n"
                        "- Define all schemas in components/schemas and use $ref\n"
                        "- Error responses must use application/problem+json with a ProblemDetails schema\n"
                        "- Reusable pagination parameters (LimitParam, OffsetParam) should be in components/parameters\n"
                        "- Use x-sse: true on GET endpoints that stream Server-Sent Events\n"
                        "- Use x-cors: inherit on endpoints that need CORS (default)\n"
                        "- Add security schemes in components/securitySchemes if authentication is needed\n\n"
                        "Start with the openapi and info blocks, then define paths, then components."
                    ),
                ),
            )
        ],
    )


def setup_bff_prompt(system_name: str, services: list[str]) -> GetPromptResult:
    """Return a prompt that helps the assistant create a BFF suite config and spec.

    Args:
        system_name: Name of the system (used as BFF title).
        services: List of downstream service names.
    """
    svc_list = "\n".join(f"  - {s}" for s in services)
    return GetPromptResult(
        description=f"Create a BFF suite config and OpenAPI spec for the '{system_name}' system",
        messages=[
            PromptMessage(
                role="user",
                content=TextContent(
                    type="text",
                    text=(
                        f"Create a BFF (Backend for Frontend) setup for the '{system_name}' system.\n"
                        f"The BFF aggregates the following downstream services:\n{svc_list}\n\n"
                        "Produce:\n"
                        "1. A bff-suite-config.yaml file with:\n"
                        "   - openapi_base_dir pointing to the openapi/ directory\n"
                        "   - output_path for the merged BFF spec\n"
                        "   - A services map with base_path and spec_path for each service\n"
                        "   - metadata: title, version, security_schemes, security\n\n"
                        "2. A brief explanation of how to generate the merged spec using:\n"
                        "   brrtrouter bff generate --suite-config bff-suite-config.yaml --validate\n\n"
                        "BFF extensions added automatically:\n"
                        "  - x-brrtrouter-downstream-path: full downstream path for the proxy\n"
                        "  - x-service: name of the owning sub-service\n"
                        "  - x-service-base-path: base path prefix\n\n"
                        "Schemas from each service are prefixed with the service name in PascalCase "
                        "(e.g. Pet → UsersPet for the users service)."
                    ),
                ),
            )
        ],
    )


def implement_handler_prompt(operation_id: str, request_type: str, response_type: str) -> GetPromptResult:
    """Return a prompt for implementing a BRRTRouter handler stub.

    Args:
        operation_id: The operationId (snake_case) of the handler to implement.
        request_type: The Rust request struct type name.
        response_type: The Rust response struct type name.
    """
    return GetPromptResult(
        description=f"Implement the '{operation_id}' handler",
        messages=[
            PromptMessage(
                role="user",
                content=TextContent(
                    type="text",
                    text=(
                        f"Implement the BRRTRouter handler for operation '{operation_id}'.\n\n"
                        f"The handler signature is:\n"
                        f"  pub fn {operation_id}(req: {request_type}) -> {response_type}\n\n"
                        "Guidelines:\n"
                        "- The file starts with `// BRRTROUTER_USER_OWNED` to prevent regeneration\n"
                        "- Access path parameters via req.path_params.<param_name>\n"
                        "- Access query parameters via req.query.<param_name> (Option<T>)\n"
                        "- Access the JSON body via req.body (Option<T>)\n"
                        "- Return the appropriate response variant for success/error cases\n"
                        "- Use `Result` return types inside and propagate errors with `?`\n"
                        "- Keep handler logic thin; delegate to a domain/service layer\n\n"
                        "Start by extracting inputs from the request, then call the business logic, "
                        "then map the result to the response type."
                    ),
                ),
            )
        ],
    )


def review_spec_prompt(spec_content: str) -> GetPromptResult:
    """Return a prompt asking the assistant to review and improve an OpenAPI spec.

    Args:
        spec_content: The OpenAPI YAML content to review.
    """
    return GetPromptResult(
        description="Review and improve an OpenAPI spec for BRRTRouter conformance",
        messages=[
            PromptMessage(
                role="user",
                content=TextContent(
                    type="text",
                    text=(
                        "Review the following OpenAPI spec for BRRTRouter conformance and suggest improvements:\n\n"
                        "```yaml\n"
                        f"{spec_content}\n"
                        "```\n\n"
                        "Check for:\n"
                        "1. openapi: 3.1.0 (required)\n"
                        "2. All operationIds are snake_case\n"
                        "3. Error responses use application/problem+json\n"
                        "4. ProblemDetails schema is defined in components/schemas\n"
                        "5. All $refs resolve within the spec\n"
                        "6. Number fields have format: decimal or format: money\n"
                        "7. SSE endpoints (x-sse: true) are on GET operations\n"
                        "8. Security schemes are defined if authentication is used\n"
                        "9. Pagination parameters use LimitParam/OffsetParam pattern\n\n"
                        "Provide a list of issues found (errors, warnings, suggestions) "
                        "and a corrected version of the spec."
                    ),
                ),
            )
        ],
    )
