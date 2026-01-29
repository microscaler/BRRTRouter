"""Tests for BFF spec generator (Story 1.4)."""

from pathlib import Path

import yaml

from brrtrouter_tooling.bff.config import load_suite_config
from brrtrouter_tooling.bff.generate import generate_bff_spec
from brrtrouter_tooling.bff.merge import merge_sub_service_specs


def test_load_suite_config(tmp_path: Path) -> None:
    """load_suite_config resolves paths relative to base_dir (default cwd)."""
    (tmp_path / "openapi" / "suite").mkdir(parents=True)
    (tmp_path / "openapi" / "suite" / "invoice").mkdir()
    spec = tmp_path / "openapi" / "suite" / "invoice" / "openapi.yaml"
    spec.write_text("openapi: 3.1.0\ninfo: { title: Invoice, version: 1.0 }\npaths: {}")
    config_file = tmp_path / "bff-suite-config.yaml"
    config_file.write_text(
        "openapi_base_dir: openapi/suite\noutput_path: openapi/suite/openapi_bff.yaml\n"
        "services:\n  invoice:\n    base_path: /api/invoice\n    spec_path: invoice/openapi.yaml\n"
    )
    config = load_suite_config(config_file, base_dir=tmp_path)
    resolved = config["_resolved"]
    assert (
        resolved["services"]["invoice"]["spec_path"]
        == tmp_path / "openapi" / "suite" / "invoice" / "openapi.yaml"
    )
    assert resolved["services"]["invoice"]["base_path"] == "/api/invoice"
    assert resolved["output_path"] == tmp_path / "openapi" / "suite" / "openapi_bff.yaml"


def test_merge_sets_proxy_extensions(tmp_path: Path) -> None:
    """Merged spec has x-service, x-service-base-path, x-brrtrouter-downstream-path on operations."""
    spec_path = tmp_path / "openapi.yaml"
    spec_path.write_text(
        "openapi: 3.1.0\ninfo: { title: S, version: 1.0 }\npaths:\n  /invoices/{id}:\n    get:\n      operationId: getInvoice\n      responses: { '200': { description: OK } }\n"
    )
    sub_services = {
        "invoice": {"base_path": "/api/invoice", "spec_path": spec_path},
    }
    bff = merge_sub_service_specs(sub_services)
    assert "paths" in bff
    assert "/invoices/{id}" in bff["paths"]
    get_op = bff["paths"]["/invoices/{id}"].get("get")
    assert get_op is not None
    assert get_op.get("x-service") == "invoice"
    assert get_op.get("x-service-base-path") == "/api/invoice"
    assert get_op.get("x-brrtrouter-downstream-path") == "/api/invoice/invoices/{id}"


def test_generate_bff_spec(tmp_path: Path) -> None:
    """generate_bff_spec produces a valid merged spec with proxy extensions."""
    (tmp_path / "openapi" / "suite" / "invoice").mkdir(parents=True)
    (tmp_path / "openapi" / "suite" / "invoice" / "openapi.yaml").write_text(
        "openapi: 3.1.0\ninfo: { title: Invoice, version: 1.0 }\npaths:\n  /invoices:\n    get:\n      operationId: listInvoices\n      responses: { '200': { description: OK } }\n"
    )
    config_file = tmp_path / "bff-suite-config.yaml"
    config_file.write_text(
        "openapi_base_dir: openapi/suite\noutput_path: openapi/suite/openapi_bff.yaml\n"
        "services:\n  invoice:\n    base_path: /api/invoice\n    spec_path: invoice/openapi.yaml\n"
    )
    out = generate_bff_spec(config_file, base_dir=tmp_path)
    assert out.exists()
    with out.open() as f:
        spec = yaml.safe_load(f)
    assert spec["openapi"] == "3.1.0"
    assert "/invoices" in spec["paths"]
    get_op = spec["paths"]["/invoices"].get("get")
    assert get_op is not None
    assert get_op.get("x-service") == "invoice"
    assert get_op.get("x-brrtrouter-downstream-path") == "/api/invoice/invoices"


def test_generate_with_security(tmp_path: Path) -> None:
    """generate_bff_spec merges metadata.security_schemes and metadata.security."""
    (tmp_path / "s" / "invoice").mkdir(parents=True)
    (tmp_path / "s" / "invoice" / "openapi.yaml").write_text(
        "openapi: 3.1.0\ninfo: { title: I, version: 1.0 }\npaths: {}\n"
    )
    config_file = tmp_path / "config.yaml"
    config_file.write_text(
        "openapi_base_dir: s\noutput_path: out.yaml\n"
        "services:\n  invoice:\n    base_path: /api/invoice\n    spec_path: invoice/openapi.yaml\n"
        "metadata:\n  security_schemes:\n    bearerAuth:\n      type: http\n      scheme: bearer\n      bearerFormat: JWT\n  security:\n  - bearerAuth: []\n"
    )
    out = generate_bff_spec(config_file, base_dir=tmp_path)
    with out.open() as f:
        spec = yaml.safe_load(f)
    assert "security" in spec
    assert spec["security"] == [{"bearerAuth": []}]
    assert spec["components"]["securitySchemes"]["bearerAuth"]["scheme"] == "bearer"
