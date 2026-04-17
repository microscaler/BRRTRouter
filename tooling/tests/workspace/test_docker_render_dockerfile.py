from pathlib import Path


def _make_openapi_spec(project_root: Path, service: str, port: int = 8002) -> None:
    d = project_root / "openapi" / service
    d.mkdir(parents=True, exist_ok=True)
    (d / "openapi.yaml").write_text(
        f"openapi: 3.1.0\ninfo: {{}}\nservers:\n  - url: http://localhost:{port}/api/v1/{service}\n"
    )


def test_render_dockerfile_template_derives_port_and_binary(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.docker.render_dockerfile import render_dockerfile_template

    _make_openapi_spec(tmp_path, "fleet", port=8002)
    (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
    (tmp_path / "openapi" / "Dockerfile.template").write_text(
        "FROM alpine\nCOPY ./build_artifacts/${TARGETARCH}/{{binary_name}} /app/{{binary_name}}\nCOPY ./microservices/hauliage/{{service_name}}/impl/config /app/config\nEXPOSE {{port}}\n"
    )

    out = render_dockerfile_template(
        tmp_path, tmp_path / "openapi" / "Dockerfile.template", "fleet"
    )

    assert "8002" in out
    assert "build_artifacts/${TARGETARCH}/fleet" in out


def test_render_dockerfile_to_path_writes_file(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.docker.render_dockerfile import render_dockerfile_to_path

    _make_openapi_spec(tmp_path, "identity", port=8001)
    d = tmp_path / "docker" / "microservices"
    d.mkdir(parents=True)
    out_path = d / "Dockerfile.identity"

    (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
    (tmp_path / "openapi" / "Dockerfile.template").write_text("EXPOSE {{port}}\n")

    render_dockerfile_to_path(
        tmp_path, tmp_path / "openapi" / "Dockerfile.template", "identity", out_path
    )

    assert out_path.exists()
    assert out_path.read_text() == "EXPOSE 8001\n"
