"""Tests for BFF spec generator (Story 1.4)."""

from pathlib import Path

import pytest
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


# --- _update_refs_in_value (migrated from RERP test_bff_generate_system) ---


class TestUpdateRefsInValue:
    """Exact ref match: old_name must not match when it is a prefix of the schema (e.g. Error vs ErrorResponse)."""

    def test_exact_match_rewritten(self) -> None:
        from brrtrouter_tooling.bff.merge import _update_refs_in_value

        val = {"$ref": "#/components/schemas/Error"}
        _update_refs_in_value(val, "Error", "ServiceError")
        assert val["$ref"] == "#/components/schemas/ServiceError"

    def test_prefix_of_schema_name_not_rewritten(self) -> None:
        from brrtrouter_tooling.bff.merge import _update_refs_in_value

        val = {"$ref": "#/components/schemas/ErrorResponse"}
        _update_refs_in_value(val, "Error", "ServiceError")
        assert val["$ref"] == "#/components/schemas/ErrorResponse"

    def test_unrelated_schema_unchanged(self) -> None:
        from brrtrouter_tooling.bff.merge import _update_refs_in_value

        val = {"$ref": "#/components/schemas/Foo"}
        _update_refs_in_value(val, "Error", "ServiceError")
        assert val["$ref"] == "#/components/schemas/Foo"


class TestMergeSameSchemaNameAcrossServices:
    """When multiple sub-services have schemas with the same name, $ref resolves to the correct service."""

    def test_refs_resolve_to_same_service_not_alphabetical_first(self, tmp_path: Path) -> None:
        # Service alpha has User (refs Address); service beta has User (refs Address).
        # Refs inside AlphaUser must become AlphaAddress; refs inside BetaUser must become BetaAddress.
        alpha_spec = tmp_path / "alpha.yaml"
        beta_spec = tmp_path / "beta.yaml"
        alpha_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: A, version: '1.0' }\npaths: {}\n"
            "components:\n  schemas:\n    Address:\n      type: object\n      properties:\n        street: { type: string }\n"
            "    User:\n      type: object\n      properties:\n        address:\n          $ref: '#/components/schemas/Address'\n"
        )
        beta_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: B, version: '1.0' }\npaths: {}\n"
            "components:\n  schemas:\n    Address:\n      type: object\n      properties:\n        city: { type: string }\n"
            "    User:\n      type: object\n      properties:\n        address:\n          $ref: '#/components/schemas/Address'\n"
        )
        sub_services = {
            "alpha": {"base_path": "/api/alpha", "spec_path": alpha_spec},
            "beta": {"base_path": "/api/beta", "spec_path": beta_spec},
        }
        bff = merge_sub_service_specs(sub_services)
        schemas = bff["components"]["schemas"]
        # AlphaUser's address ref must point to AlphaAddress, not BetaAddress
        alpha_user = schemas.get("AlphaUser")
        assert alpha_user is not None
        addr_ref = alpha_user.get("properties", {}).get("address", {}).get("$ref")
        assert addr_ref == "#/components/schemas/AlphaAddress"
        # BetaUser's address ref must point to BetaAddress, not AlphaAddress
        beta_user = schemas.get("BetaUser")
        assert beta_user is not None
        addr_ref_beta = beta_user.get("properties", {}).get("address", {}).get("$ref")
        assert addr_ref_beta == "#/components/schemas/BetaAddress"


class TestMergeOverlappingPrefixes:
    """When service names produce overlapping PascalCase prefixes (Account vs Accounting), longest match wins."""

    def test_longest_prefix_used_for_schema_and_refs(self, tmp_path: Path) -> None:
        # account -> Account, accounting -> Accounting. AccountingUser must map to Accounting, not Account.
        account_spec = tmp_path / "account.yaml"
        accounting_spec = tmp_path / "accounting.yaml"
        account_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: A, version: '1.0' }\npaths: {}\n"
            "components:\n  schemas:\n    User:\n      type: object\n      properties:\n        id: { type: string }\n"
        )
        accounting_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: B, version: '1.0' }\npaths: {}\n"
            "components:\n  schemas:\n    Address:\n      type: object\n      properties:\n        city: { type: string }\n"
            "    User:\n      type: object\n      properties:\n        address:\n          $ref: '#/components/schemas/Address'\n"
        )
        sub_services = {
            "account": {"base_path": "/api/account", "spec_path": account_spec},
            "accounting": {"base_path": "/api/accounting", "spec_path": accounting_spec},
        }
        bff = merge_sub_service_specs(sub_services)
        schemas = bff["components"]["schemas"]
        # AccountingUser must resolve Address ref to AccountingAddress (longest prefix), not AccountAddress
        accounting_user = schemas.get("AccountingUser")
        assert accounting_user is not None
        addr_ref = accounting_user.get("properties", {}).get("address", {}).get("$ref")
        assert addr_ref == "#/components/schemas/AccountingAddress"
        # Both AccountUser and AccountingUser must exist
        assert "AccountUser" in schemas
        assert "AccountingUser" in schemas
        assert "AccountingAddress" in schemas


class TestMergePathsPerMethod:
    """Paths are merged per HTTP method; duplicate path+method from different services raises."""

    def test_same_path_different_methods_merged(self, tmp_path: Path) -> None:
        # Service A: GET /items; Service B: POST /items -> merged path has both.
        a_spec = tmp_path / "a.yaml"
        b_spec = tmp_path / "b.yaml"
        a_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: A, version: '1.0' }\npaths:\n  /items:\n    get:\n      operationId: listItems\n      responses: { '200': { description: OK } }\n"
        )
        b_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: B, version: '1.0' }\npaths:\n  /items:\n    post:\n      operationId: createItem\n      responses: { '201': { description: Created } }\n"
        )
        sub_services = {
            "alpha": {"base_path": "/api/alpha", "spec_path": a_spec},
            "beta": {"base_path": "/api/beta", "spec_path": b_spec},
        }
        bff = merge_sub_service_specs(sub_services)
        path_def = bff["paths"].get("/items")
        assert path_def is not None
        assert "get" in path_def and path_def["get"].get("x-service") == "alpha"
        assert "post" in path_def and path_def["post"].get("x-service") == "beta"

    def test_same_path_same_method_different_services_raises(self, tmp_path: Path) -> None:
        # Both services define GET /items -> ValueError.
        a_spec = tmp_path / "a.yaml"
        b_spec = tmp_path / "b.yaml"
        a_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: A, version: '1.0' }\npaths:\n  /items:\n    get:\n      operationId: listA\n      responses: { '200': { description: OK } }\n"
        )
        b_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: B, version: '1.0' }\npaths:\n  /items:\n    get:\n      operationId: listB\n      responses: { '200': { description: OK } }\n"
        )
        sub_services = {
            "alpha": {"base_path": "/api/alpha", "spec_path": a_spec},
            "beta": {"base_path": "/api/beta", "spec_path": b_spec},
        }
        with pytest.raises(ValueError) as exc_info:
            merge_sub_service_specs(sub_services)
        assert "/items" in str(exc_info.value)
        assert "get" in str(exc_info.value).lower() or "method" in str(exc_info.value).lower()


class TestMergeMultipleErrorSchemas:
    """When multiple sub-services define a per-service Error schema, merge raises."""

    def test_multiple_error_schemas_raises(self, tmp_path: Path) -> None:
        # Both auth and idam define Error; after merge we get AuthError and IdamError -> ambiguous.
        auth_spec = tmp_path / "auth.yaml"
        idam_spec = tmp_path / "idam.yaml"
        auth_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: Auth, version: '1.0' }\npaths: {}\n"
            "components:\n  schemas:\n    Error:\n      type: object\n      properties:\n        code: { type: string }\n"
        )
        idam_spec.write_text(
            "openapi: 3.1.0\ninfo: { title: Idam, version: '1.0' }\npaths: {}\n"
            "components:\n  schemas:\n    Error:\n      type: object\n      properties:\n        code: { type: string }\n"
        )
        sub_services = {
            "auth": {"base_path": "/api/auth", "spec_path": auth_spec},
            "idam": {"base_path": "/api/idam", "spec_path": idam_spec},
        }
        with pytest.raises(ValueError) as exc_info:
            merge_sub_service_specs(sub_services)
        msg = str(exc_info.value)
        assert "Multiple service Error schemas" in msg
        assert "AuthError" in msg
        assert "IdamError" in msg


# --- discover_sub_services (migrated from RERP) ---


class TestDiscoverSubServices:
    def test_system_dir_missing_returns_empty(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        got = discover_sub_services(tmp_path, "nosuch")
        assert got == {}

    def test_no_subdirs_with_openapi_returns_empty(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        (tmp_path / "foo").mkdir()
        got = discover_sub_services(tmp_path, "foo")
        assert got == {}

    def test_one_subdir_with_openapi_returns_service(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        (tmp_path / "sys" / "svc").mkdir(parents=True)
        (tmp_path / "sys" / "svc" / "openapi.yaml").write_text(
            "openapi: 3.1.0\ninfo: { title: X, version: '1.0' }\npaths: {}"
        )
        got = discover_sub_services(tmp_path, "sys")
        assert list(got.keys()) == ["svc"]
        assert got["svc"]["spec_path"] == tmp_path / "sys" / "svc" / "openapi.yaml"
        assert got["svc"]["base_path"] == "/api/v1/sys/svc"

    def test_two_subdirs_returns_both(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        for s in ["a", "b"]:
            (tmp_path / "s" / s).mkdir(parents=True)
            (tmp_path / "s" / s / "openapi.yaml").write_text("openapi: 3.1.0\ninfo: {}\npaths: {}")
        got = discover_sub_services(tmp_path, "s")
        assert set(got.keys()) == {"a", "b"}
        assert got["a"]["base_path"] == "/api/v1/s/a"
        assert got["b"]["base_path"] == "/api/v1/s/b"

    def test_skips_hidden_dirs(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        (tmp_path / "s" / ".hidden").mkdir(parents=True)
        (tmp_path / "s" / ".hidden" / "openapi.yaml").write_text(
            "openapi: 3.1.0\ninfo: {}\npaths: {}"
        )
        (tmp_path / "s" / "ok").mkdir()
        (tmp_path / "s" / "ok" / "openapi.yaml").write_text("openapi: 3.1.0\ninfo: {}\npaths: {}")
        got = discover_sub_services(tmp_path, "s")
        assert list(got.keys()) == ["ok"]

    def test_skips_dirs_without_openapi_yaml(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        (tmp_path / "s" / "no-spec").mkdir(parents=True)
        (tmp_path / "s" / "with-spec").mkdir()
        (tmp_path / "s" / "with-spec" / "openapi.yaml").write_text(
            "openapi: 3.1.0\ninfo: {}\npaths: {}"
        )
        got = discover_sub_services(tmp_path, "s")
        assert list(got.keys()) == ["with-spec"]

    def test_base_path_uses_system_and_service(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import discover_sub_services

        (tmp_path / "accounting" / "general-ledger").mkdir(parents=True)
        (tmp_path / "accounting" / "general-ledger" / "openapi.yaml").write_text(
            "openapi: 3.1.0\ninfo: {}\npaths: {}"
        )
        got = discover_sub_services(tmp_path, "accounting")
        assert got["general-ledger"]["base_path"] == "/api/v1/accounting/general-ledger"


# --- list_systems_with_sub_services (migrated from RERP) ---


class TestListSystemsWithSubServices:
    def test_empty_openapi_dir_returns_empty(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import list_systems_with_sub_services

        assert list_systems_with_sub_services(tmp_path) == []

    def test_system_with_sub_services_included(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import list_systems_with_sub_services

        (tmp_path / "x" / "a").mkdir(parents=True)
        (tmp_path / "x" / "a" / "openapi.yaml").write_text("openapi: 3.1.0\ninfo: {}\npaths: {}")
        assert list_systems_with_sub_services(tmp_path) == ["x"]

    def test_system_without_sub_services_excluded(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import list_systems_with_sub_services

        (tmp_path / "empty").mkdir()
        (tmp_path / "with_subs" / "a").mkdir(parents=True)
        (tmp_path / "with_subs" / "a" / "openapi.yaml").write_text(
            "openapi: 3.1.0\ninfo: {}\npaths: {}"
        )
        got = list_systems_with_sub_services(tmp_path)
        assert got == ["with_subs"]

    def test_multiple_systems_sorted(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.discovery import list_systems_with_sub_services

        for sys in ["b", "a", "c"]:
            (tmp_path / sys / "x").mkdir(parents=True)
            (tmp_path / sys / "x" / "openapi.yaml").write_text(
                "openapi: 3.1.0\ninfo: {}\npaths: {}"
            )
        assert list_systems_with_sub_services(tmp_path) == ["a", "b", "c"]


# --- generate_system_bff_spec (migrated from RERP) ---


def _minimal_spec(paths: dict | None = None, schemas: dict | None = None) -> str:
    p = paths if paths is not None else {}
    s = schemas if schemas is not None else {}
    o = {"openapi": "3.1.0", "info": {"title": "T", "version": "1.0"}, "paths": p}
    if s:
        o["components"] = {"schemas": s}
    return yaml.dump(o, sort_keys=False)


class TestGenerateSystemBffSpec:
    def test_no_sub_services_does_not_write(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec

        out = tmp_path / "s" / "openapi.yaml"
        generate_system_bff_spec(tmp_path, "s", output_path=out)
        assert not out.exists()

    def test_one_sub_service_writes_valid_spec(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec

        (tmp_path / "sys" / "svc").mkdir(parents=True)
        (tmp_path / "sys" / "svc" / "openapi.yaml").write_text(
            _minimal_spec(
                paths={
                    "/items": {
                        "get": {"summary": "List", "responses": {"200": {"description": "ok"}}}
                    }
                }
            )
        )
        out = tmp_path / "sys" / "openapi.yaml"
        generate_system_bff_spec(tmp_path, "sys", output_path=out)
        assert out.exists()
        data = yaml.safe_load(out.read_text())
        assert data["openapi"] == "3.1.0"
        assert "paths" in data
        assert "/items" in data["paths"]
        assert "components" in data
        assert "schemas" in data["components"]

    def test_output_path_default_when_none(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec

        (tmp_path / "x" / "a").mkdir(parents=True)
        (tmp_path / "x" / "a" / "openapi.yaml").write_text(_minimal_spec())
        generate_system_bff_spec(tmp_path, "x", output_path=None)
        default = tmp_path / "x" / "openapi.yaml"
        assert default.exists()

    def test_idempotent_second_run_same_content(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec

        (tmp_path / "s" / "v").mkdir(parents=True)
        (tmp_path / "s" / "v" / "openapi.yaml").write_text(_minimal_spec())
        out = tmp_path / "s" / "openapi.yaml"
        generate_system_bff_spec(tmp_path, "s", output_path=out)
        c1 = out.read_text()
        generate_system_bff_spec(tmp_path, "s", output_path=out)
        c2 = out.read_text()
        assert c1 == c2

    def test_schemas_prefixed_with_service_pascal(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec

        (tmp_path / "s" / "my-svc").mkdir(parents=True)
        (tmp_path / "s" / "my-svc" / "openapi.yaml").write_text(
            _minimal_spec(
                schemas={"Item": {"type": "object", "properties": {"id": {"type": "string"}}}}
            )
        )
        out = tmp_path / "s" / "openapi.yaml"
        generate_system_bff_spec(tmp_path, "s", output_path=out)
        data = yaml.safe_load(out.read_text())
        schemas = data["components"]["schemas"]
        assert "MySvcItem" in schemas
        assert schemas["MySvcItem"].get("type") == "object"
