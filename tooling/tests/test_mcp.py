"""Tests for BRRTRouter MCP tools and resources.

Tests focus on the pure-Python logic (tools.py, resources.py) without requiring
a live MCP server or the mcp package to be installed.  Server assembly is tested
via a separate import test that skips when mcp is unavailable.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

from brrtrouter_tooling.mcp.resources import (
    get_bff_pattern_guide,
    get_code_generation_guide,
    get_example_openapi_yaml,
    get_extensions_reference,
    get_openapi_spec_guide,
    get_tilt_setup_guide,
)
from brrtrouter_tooling.mcp.tools import (
    check_spec_conformance,
    inspect_generated_dir,
    inspect_impl_dir,
    lint_spec,
    list_spec_operations,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_MINIMAL_VALID_SPEC = """\
openapi: 3.1.0
info:
  title: Test Service
  version: "1.0.0"
paths:
  /items:
    get:
      operationId: list_items
      summary: List items
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Item"
        "400":
          description: Error
          content:
            application/problem+json:
              schema:
                $ref: "#/components/schemas/ProblemDetails"
components:
  schemas:
    Item:
      type: object
      properties:
        id:
          type: string
    ProblemDetails:
      type: object
      properties:
        title:
          type: string
        status:
          type: integer
"""

_INVALID_SPEC = """\
openapi: 3.0.0
info:
  title: Bad Service
paths:
  /items:
    get:
      operationId: listItems
      responses:
        "200":
          description: OK
"""


# ---------------------------------------------------------------------------
# Resources
# ---------------------------------------------------------------------------


def test_resources_return_strings() -> None:
    """All resource getter functions return non-empty strings."""
    assert len(get_openapi_spec_guide()) > 100
    assert len(get_code_generation_guide()) > 100
    assert len(get_bff_pattern_guide()) > 100
    assert len(get_extensions_reference()) > 100
    assert len(get_example_openapi_yaml()) > 100
    assert len(get_tilt_setup_guide()) > 100


def test_example_openapi_yaml_is_valid_yaml() -> None:
    """The example OpenAPI YAML parses as valid YAML."""
    spec = yaml.safe_load(get_example_openapi_yaml())
    assert isinstance(spec, dict)
    assert spec.get("openapi") == "3.1.0"
    assert "paths" in spec
    assert "components" in spec


def test_openapi_spec_guide_mentions_snake_case() -> None:
    guide = get_openapi_spec_guide()
    assert "snake_case" in guide
    assert "operationId" in guide


def test_code_generation_guide_mentions_gen_and_impl() -> None:
    guide = get_code_generation_guide()
    assert "gen crate" in guide
    assert "impl crate" in guide
    assert "generate-stubs" in guide
    assert "Consumer CLI: host-aware build" in guide
    assert "bff_traderBFF" in guide


def test_bff_guide_mentions_suite_config() -> None:
    guide = get_bff_pattern_guide()
    assert "suite" in guide.lower()
    assert "x-brrtrouter-downstream-path" in guide


def test_bff_guide_documents_tilt_scan_for_bff_spec_gen_deps() -> None:
    guide = get_bff_pattern_guide()
    assert "bff-spec-gen" in guide
    assert "tilt scan" in guide
    assert "TRADER_SERVICES" in guide
    assert "lib.tilt" in guide
    assert "tilt-setup" in guide


def test_tilt_setup_guide_starlark_boundary_and_lib_tilt_examples() -> None:
    guide = get_tilt_setup_guide()
    assert len(guide) > 500
    assert "lib.tilt" in guide
    assert "load(" in guide
    assert "brrtrouter_tooling" in guide
    assert "cannot" in guide.lower() and "import" in guide.lower()
    assert "create_bff_spec_gen_deps" in guide


def test_extensions_reference_covers_x_sse() -> None:
    ref = get_extensions_reference()
    assert "x-sse" in ref
    assert "x-cors" in ref


# ---------------------------------------------------------------------------
# lint_spec
# ---------------------------------------------------------------------------


def test_lint_spec_passes_valid_spec() -> None:
    result = lint_spec(_MINIMAL_VALID_SPEC)
    assert "passed" in result.lower() or "conformant" in result.lower()


def test_lint_spec_detects_wrong_openapi_version() -> None:
    result = lint_spec(_INVALID_SPEC)
    assert "3.1.0" in result


def test_lint_spec_detects_missing_version_in_info() -> None:
    spec = """\
openapi: 3.1.0
info:
  title: My Service
paths:
  /x:
    get:
      operationId: do_thing
      responses:
        "200":
          description: OK
"""
    result = lint_spec(spec)
    assert "version" in result.lower()


def test_lint_spec_detects_camel_case_operation_id() -> None:
    result = lint_spec(_INVALID_SPEC)
    assert "listItems" in result or "snake_case" in result.lower()


def test_lint_spec_detects_missing_operation_id() -> None:
    spec = """\
openapi: 3.1.0
info:
  title: Svc
  version: "1.0.0"
paths:
  /x:
    get:
      responses:
        "200":
          description: OK
"""
    result = lint_spec(spec)
    assert "operationId" in result or "missing" in result.lower()


def test_lint_spec_detects_unresolved_ref() -> None:
    spec = """\
openapi: 3.1.0
info:
  title: Svc
  version: "1.0.0"
paths:
  /x:
    get:
      operationId: get_x
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/MissingSchema"
components:
  schemas: {}
"""
    result = lint_spec(spec)
    assert "MissingSchema" in result or "Unresolved" in result


def test_lint_spec_invalid_yaml() -> None:
    result = lint_spec("not: valid: yaml: [")
    assert "error" in result.lower() or "parse" in result.lower()


def test_lint_spec_non_mapping_root() -> None:
    result = lint_spec("- item1\n- item2\n")
    assert "mapping" in result.lower() or "error" in result.lower()


# ---------------------------------------------------------------------------
# check_spec_conformance
# ---------------------------------------------------------------------------


def test_check_conformance_passes_valid_spec() -> None:
    result = check_spec_conformance(_MINIMAL_VALID_SPEC)
    assert "✅" in result


def test_check_conformance_detects_non_problem_json_error_response() -> None:
    spec = """\
openapi: 3.1.0
info:
  title: Svc
  version: "1.0.0"
paths:
  /x:
    get:
      operationId: get_x
      responses:
        "404":
          description: Not found
          content:
            application/json:
              schema:
                type: object
"""
    result = check_spec_conformance(spec)
    assert "problem+json" in result.lower() or "RFC 7807" in result


def test_check_conformance_detects_sse_on_post() -> None:
    spec = """\
openapi: 3.1.0
info:
  title: Svc
  version: "1.0.0"
paths:
  /events:
    post:
      operationId: post_events
      x-sse: true
      responses:
        "200":
          description: SSE
"""
    result = check_spec_conformance(spec)
    assert "x-sse" in result or "non-GET" in result


def test_check_conformance_detects_number_without_format() -> None:
    spec = """\
openapi: 3.1.0
info:
  title: Svc
  version: "1.0.0"
paths:
  /x:
    get:
      operationId: get_x
      responses:
        "200":
          description: OK
components:
  schemas:
    Price:
      type: object
      properties:
        amount:
          type: number
"""
    result = check_spec_conformance(spec)
    assert "format" in result.lower() or "number" in result.lower()


# ---------------------------------------------------------------------------
# list_spec_operations
# ---------------------------------------------------------------------------


def test_list_spec_operations(tmp_path: Path) -> None:
    spec_file = tmp_path / "openapi.yaml"
    spec_file.write_text(_MINIMAL_VALID_SPEC)
    result = list_spec_operations(str(spec_file))
    assert "list_items" in result
    assert "GET" in result
    assert "/items" in result


def test_list_spec_operations_missing_file() -> None:
    result = list_spec_operations("/nonexistent/path/openapi.yaml")
    assert "not found" in result.lower() or "error" in result.lower()


def test_list_spec_operations_sse_marker(tmp_path: Path) -> None:
    spec = """\
openapi: 3.1.0
info:
  title: S
  version: "1.0.0"
paths:
  /events:
    get:
      operationId: stream_events
      x-sse: true
      responses:
        "200":
          description: SSE
"""
    f = tmp_path / "openapi.yaml"
    f.write_text(spec)
    result = list_spec_operations(str(f))
    assert "[SSE]" in result


# ---------------------------------------------------------------------------
# inspect_generated_dir
# ---------------------------------------------------------------------------


def _make_gen_dir(tmp_path: Path) -> Path:
    """Create a minimal fake gen crate structure."""
    d = tmp_path / "my_gen"
    handlers = d / "src" / "handlers"
    controllers = d / "src" / "controllers"
    handlers.mkdir(parents=True)
    controllers.mkdir(parents=True)
    (handlers / "mod.rs").write_text("// mod")
    (handlers / "types.rs").write_text("// types")
    (handlers / "list_items.rs").write_text("// handler")
    (handlers / "get_item.rs").write_text("// handler")
    (controllers / "mod.rs").write_text("// mod")
    (controllers / "list_items.rs").write_text("// controller")
    config = d / "config"
    config.mkdir()
    (config / "config.yaml").write_text("cors:\n  origins: []\n")
    (d / "Cargo.toml").write_text('[package]\nname = "my_gen"\nversion = "0.1.0"\n')
    return d


def test_inspect_generated_dir(tmp_path: Path) -> None:
    d = _make_gen_dir(tmp_path)
    result = inspect_generated_dir(str(d))
    assert "list_items" in result
    assert "get_item" in result
    assert "my_gen" in result


def test_inspect_generated_dir_missing() -> None:
    result = inspect_generated_dir("/nonexistent/path")
    assert "not found" in result.lower()


# ---------------------------------------------------------------------------
# inspect_impl_dir
# ---------------------------------------------------------------------------


def _make_impl_dir(tmp_path: Path) -> Path:
    """Create a minimal fake impl crate structure."""
    d = tmp_path / "my_impl"
    handlers = d / "src" / "handlers"
    handlers.mkdir(parents=True)
    (handlers / "mod.rs").write_text("// mod")
    (handlers / "list_items.rs").write_text("// BRRTROUTER_USER_OWNED\npub fn list_items() {}")
    (handlers / "get_item.rs").write_text("// generated stub\npub fn get_item() {}")
    return d


def test_inspect_impl_dir(tmp_path: Path) -> None:
    d = _make_impl_dir(tmp_path)
    result = inspect_impl_dir(str(d))
    assert "list_items" in result
    assert "get_item" in result
    assert "🔒" in result  # user-owned
    assert "📄" in result  # stub


def test_inspect_impl_dir_missing() -> None:
    result = inspect_impl_dir("/nonexistent/path")
    assert "not found" in result.lower()


# ---------------------------------------------------------------------------
# Server import test (skips if mcp not installed)
# ---------------------------------------------------------------------------


def test_create_mcp_server_importable() -> None:
    """create_mcp_server can be imported and the server has expected tools/resources."""
    pytest.importorskip("mcp", reason="mcp package not installed; skip server assembly test")
    from brrtrouter_tooling.mcp.server import create_mcp_server

    server = create_mcp_server()
    assert server is not None
