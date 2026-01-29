"""Bootstrap a new microservice crate from an OpenAPI specification.

Creates crate (via BRRTRouter), Dockerfile, config, workspace Cargo.toml, Tiltfile.
Layout is configurable (openapi_dir, suite, workspace_dir, docker_dir, tiltfile, crate_name_prefix).
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout
from brrtrouter_tooling.bootstrap.helpers import (
    _get_port_from_registry,
    derive_binary_name,
)
from brrtrouter_tooling.helpers import load_yaml_spec, to_pascal_case

# Initial version for generated *_gen crates (conventional 0.1.0 for first pre-1.0 release).
GEN_CRATE_INITIAL_VERSION = "0.1.0"


def create_dockerfile(
    service_name: str,
    binary_name: str,
    port: int,
    output_path: Path,
    workspace_dir: str,
    suite: str,
) -> None:
    content = f"""# Minimal runtime-only Dockerfile for {to_pascal_case(service_name)} Service (Tilt development)
# Binary is cross-compiled on host and copied in

ARG TARGETPLATFORM=linux/amd64
FROM --platform=${{TARGETPLATFORM}} alpine:3.19

RUN apk add --no-cache \\
    ca-certificates \\
    libgcc

WORKDIR /app

COPY ./build_artifacts/{binary_name} /app/{binary_name}
RUN chmod +x /app/{binary_name}

RUN mkdir -p /app/config /app/doc /app/static_site && \\
    chmod -R 755 /app

COPY ./{workspace_dir}/{suite}/{service_name}/impl/config /app/config
COPY ./{workspace_dir}/{suite}/{service_name}/gen/doc /app/doc
COPY ./{workspace_dir}/{suite}/{service_name}/gen/static_site /app/static_site

EXPOSE {port}

ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug

ENTRYPOINT ["/app/{binary_name}", \\
    "--spec", "/app/doc/openapi.yaml", \\
    "--doc-dir", "/app/doc", \\
    "--static-dir", "/app/static_site", \\
    "--config", "/app/config/config.yaml"]
"""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(content)
    print(f"✅ Created Dockerfile: {output_path}")


def create_config_yaml(output_path: Path) -> None:
    config_content = """# BRRTRouter application configuration (YAML)
security:
  api_keys:
    ApiKeyHeader:
      key: "test123"
http:
  keep_alive: true
  timeout_secs: 5
  max_requests: 5000
"""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(config_content)
    print(f"✅ Created config.yaml: {output_path}")


def create_dependencies_config_toml(output_path: Path) -> None:
    config_content = """# BRRTRouter Dependencies Configuration
[dependencies]
[conditional]
rust_decimal = { detect = "rust_decimal::Decimal", workspace = true }
"""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(config_content)
    print(f"✅ Created brrtrouter-dependencies.toml: {output_path}")


def update_workspace_cargo_toml(service_name: str, cargo_toml_path: Path, suite: str) -> None:
    if not cargo_toml_path.exists():
        return
    content = cargo_toml_path.read_text()
    gen_member = f'"{suite}/{service_name}/gen"'
    impl_member = f'"{suite}/{service_name}/impl"'
    if gen_member in content and impl_member in content:
        return
    m = re.search(r"(members\s*=\s*\[)(.*?)(\])", content, re.DOTALL)
    if not m:
        return
    existing = [x.strip().strip('"') for x in m.group(2).split(",") if x.strip()]
    if gen_member.strip('"') not in existing:
        existing.append(gen_member.strip('"'))
    if impl_member.strip('"') not in existing:
        existing.append(impl_member.strip('"'))
    existing.sort()
    new_members = '    "' + '",\n    "'.join(existing) + '",\n'
    new_content = content[: m.start()] + m.group(1) + "\n" + new_members + "]" + content[m.end() :]
    cargo_toml_path.write_text(new_content)
    print(f"✅ Added {service_name}/gen and {service_name}/impl to workspace Cargo.toml")


def update_tiltfile(
    service_name: str,
    spec_file: str,
    binary_name: str,
    port: int,
    tiltfile_path: Path,
    workspace_dir: str,
    suite: str,
) -> None:
    if not tiltfile_path.exists():
        return
    content = tiltfile_path.read_text()
    orig = content

    m = re.search(r"(BINARY_NAMES\s*=\s*\{)(.*?)(\})", content, re.DOTALL)
    if m and f"'{service_name}':" not in m.group(2) and f'"{service_name}":' not in m.group(2):
        entries = [
            line.strip()
            for line in m.group(2).split("\n")
            if line.strip() and not line.strip().startswith("#")
        ]
        entries.append(f"'{service_name}': '{binary_name}',")
        entries.sort()
        content = (
            content[: m.start()]
            + m.group(1)
            + "\n"
            + "\n".join("    " + e for e in entries)
            + "\n}"
            + content[m.end() :]
        )

    lint_call = f"create_microservice_lint('{service_name}', '{spec_file}')"
    if lint_call not in content:
        for m in list(re.finditer(r"(create_microservice_lint\([^\n]+\n)", content))[::-1]:
            content = content[: m.end()] + lint_call + "\n" + content[m.end() :]
            break

    gen_call = f"create_microservice_gen('{service_name}', '{spec_file}', '{service_name}')"
    if gen_call not in content:
        for m in list(re.finditer(r"(create_microservice_gen\([^\n]+\n)", content))[::-1]:
            content = content[: m.end()] + gen_call + "\n" + content[m.end() :]
            break

    m = re.search(
        r"(resource_deps=\[)(.*?)(\]\s*labels=\['microservices-build'\])",
        content,
        re.DOTALL,
    )
    if m and f"'{service_name}-service-gen'" not in m.group(2):
        deps = [d.strip().strip("'\"") for d in m.group(2).split(",") if d.strip()]
        deps.append(f"{service_name}-service-gen")
        deps.sort()
        content = (
            content[: m.start()]
            + m.group(1)
            + "'"
            + "', '".join(deps)
            + "',\n    "
            + m.group(3)
            + content[m.end() :]
        )

    m = re.search(r"(deps=\[)(.*?)(\]\s*resource_deps=)", content, re.DOTALL)
    gen_cargo = f"'./{workspace_dir}/{suite}/{service_name}/gen/Cargo.toml'"
    impl_cargo = f"'./{workspace_dir}/{suite}/{service_name}/impl/Cargo.toml'"
    if m:
        deps = [d.strip().strip("'\"") for d in m.group(2).split(",") if d.strip()]
        if gen_cargo.strip("'\"") not in deps:
            deps.append(gen_cargo.strip("'\""))
        if impl_cargo.strip("'\"") not in deps:
            deps.append(impl_cargo.strip("'\""))
        deps.sort()
        content = (
            content[: m.start()]
            + m.group(1)
            + "'"
            + "',\n        '".join(deps)
            + "',\n    "
            + m.group(3)
            + content[m.end() :]
        )

    m = re.search(r"(ports\s*=\s*\{)(.*?)(\s*\})", content, re.DOTALL)
    if m and f"'{service_name}':" not in m.group(2):
        lines = [
            line.strip()
            for line in m.group(2).split("\n")
            if line.strip() and not line.strip().startswith("#")
        ]
        lines.append(f"'{service_name}': '{port}',")
        lines.sort()
        content = (
            content[: m.start()]
            + m.group(1)
            + "\n"
            + "\n".join("        " + e for e in lines)
            + m.group(3)
            + content[m.end() :]
        )

    deployment_call = f"create_microservice_deployment('{service_name}')"
    if deployment_call not in content:
        for m in list(re.finditer(r"(create_microservice_deployment\([^\n]+\n)", content))[::-1]:
            content = content[: m.end()] + deployment_call + "\n" + content[m.end() :]
            break

    if content != orig:
        tiltfile_path.write_text(content)
        print("✅ Updated Tiltfile")


def _update_gen_cargo_toml(cargo_path: Path, service_name: str, crate_name_prefix: str) -> None:
    content = cargo_path.read_text()
    service_snake = service_name.replace("-", "_")
    gen_crate_name = f"{crate_name_prefix}_{service_snake}_gen"

    content = re.sub(r'name = "[^"]+"', f'name = "{gen_crate_name}"', content, count=1)
    content = re.sub(
        r'version = "[^"]+"',
        f'version = "{GEN_CRATE_INITIAL_VERSION}"',
        content,
        count=1,
    )

    if "[lib]" not in content:
        content = re.sub(
            r"(\[package\][^\[]+)",
            r'\1\n[lib]\nname = "' + gen_crate_name + '"\npath = "src/lib.rs"\n',
            content,
            count=1,
        )

    gen_src_dir = cargo_path.parent / "src"
    uses_money = False
    uses_decimal = False
    if gen_src_dir.exists():
        for rust_file in gen_src_dir.rglob("*.rs"):
            try:
                fc = rust_file.read_text()
                if "rusty_money::Money" in fc or "Money<" in fc:
                    uses_money = True
                if "rust_decimal::Decimal" in fc or re.search(
                    r":\s*Decimal\b|<Decimal>|Decimal::", fc
                ):
                    uses_decimal = True
                if uses_money and uses_decimal:
                    break
            except (OSError, UnicodeDecodeError):
                continue

    if uses_money and "rusty-money" not in content:
        if "tikv-jemallocator" in content:
            content = re.sub(
                r"(tikv-jemallocator = \{[^\}]+\}\n)",
                r"\1rusty-money = { workspace = true }\n",
                content,
                count=1,
            )
        else:
            content = re.sub(
                r"(\[dependencies\][^\[]+)",
                r"\1rusty-money = { workspace = true }\n",
                content,
                count=1,
            )
    if uses_decimal and "rust_decimal" not in content:
        if "rusty-money" in content:
            content = re.sub(
                r"(rusty-money = \{[^\}]+\}\n)",
                r"\1rust_decimal = { workspace = true }\n",
                content,
                count=1,
            )
        elif "tikv-jemallocator" in content:
            content = re.sub(
                r"(tikv-jemallocator = \{[^\}]+\}\n)",
                r"\1rust_decimal = { workspace = true }\n",
                content,
                count=1,
            )
        else:
            content = re.sub(
                r"(\[dependencies\][^\[]+)",
                r"\1rust_decimal = { workspace = true }\n",
                content,
                count=1,
            )
    cargo_path.write_text(content)


def _generate_impl_with_brrtrouter(
    spec_path: Path,
    impl_dir: Path,
    service_name: str,
    project_root: Path,
    crate_name_prefix: str,
) -> None:
    from brrtrouter_tooling.gen import call_brrtrouter_generate_stubs

    service_snake = service_name.replace("-", "_")
    gen_crate_name = f"{crate_name_prefix}_{service_snake}_gen"
    impl_dir.mkdir(parents=True, exist_ok=True)

    result = call_brrtrouter_generate_stubs(
        spec_path=spec_path,
        impl_dir=impl_dir,
        component_name=gen_crate_name,
        project_root=project_root,
        force=True,
        capture_output=True,
    )
    if result.returncode != 0:
        msg = "BRRTRouter impl generation failed: " + str(result.stderr)
        raise RuntimeError(msg)

    impl_cargo = impl_dir / "Cargo.toml"
    if impl_cargo.exists():
        _fix_impl_cargo_naming(impl_cargo, service_name, crate_name_prefix)
    impl_main = impl_dir / "src" / "main.rs"
    gen_cargo = impl_dir.parent / "gen" / "Cargo.toml"
    if impl_main.exists():
        _fix_impl_main_naming(impl_main, service_name, crate_name_prefix, gen_cargo_toml=gen_cargo)
    print("✅ Generated impl crate with BRRTRouter")


def _fix_impl_cargo_naming(cargo_path: Path, service_name: str, crate_name_prefix: str) -> None:
    if not cargo_path.exists():
        return
    service_snake = service_name.replace("-", "_")
    gen_crate_name = f"{crate_name_prefix}_{service_snake}_gen"
    impl_crate_name = f"{crate_name_prefix}_{service_snake}"
    content = cargo_path.read_text()
    if f'name = "{impl_crate_name}"' not in content:
        content = re.sub(
            r'name = "[^"]+"',
            f'name = "{impl_crate_name}"',
            content,
            count=1,
        )
    gen_dep_pattern = r'^(\w+) = \{ path = "\.\./[^"]+" \}'
    if re.search(gen_dep_pattern, content, re.MULTILINE):
        content = re.sub(
            gen_dep_pattern,
            f'{gen_crate_name} = {{ path = "../gen" }}',
            content,
            count=1,
            flags=re.MULTILINE,
        )
    cargo_path.write_text(content)


def _read_gen_crate_name_from_cargo_toml(gen_cargo_toml: Path) -> str | None:
    """Read [package] name from gen Cargo.toml. Returns None if not found."""
    if not gen_cargo_toml.exists():
        return None
    text = gen_cargo_toml.read_text()
    in_package = False
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("["):
            in_package = s.strip("[]").strip() == "package"
            continue
        if in_package:
            m = re.match(r'name\s*=\s*"([^"]+)"', line)
            if m:
                return m.group(1)
    return None


def _fix_impl_main_naming(
    main_path: Path,
    service_name: str,
    crate_name_prefix: str,
    gen_cargo_toml: Path | None = None,
) -> None:
    """Replace only the gen-crate use (e.g. use pet_store_gen::) with the actual gen crate name.

    Do not replace std::, serde::, brrtrouter::, clap::, etc.
    """
    if not main_path.exists():
        return
    service_snake = service_name.replace("-", "_")
    gen_crate_name = f"{crate_name_prefix}_{service_snake}_gen"
    content = main_path.read_text()
    original_gen_name = (
        _read_gen_crate_name_from_cargo_toml(gen_cargo_toml) if gen_cargo_toml else None
    )
    if original_gen_name:
        content = re.sub(
            re.escape(f"use {original_gen_name}::"),
            f"use {gen_crate_name}::",
            content,
        )
    else:
        content = re.sub(r"use \w+_gen::", f"use {gen_crate_name}::", content)
    main_path.write_text(content)


def generate_code_with_brrtrouter(spec_path: Path, output_dir: Path, project_root: Path) -> None:
    from brrtrouter_tooling.gen import call_brrtrouter_generate

    deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"
    deps_config = deps_config_path if deps_config_path.exists() else None
    result = call_brrtrouter_generate(
        spec_path=spec_path,
        output_dir=output_dir,
        project_root=project_root,
        deps_config_path=deps_config,
        capture_output=True,
    )
    if result.returncode != 0:
        msg = "BRRTRouter generation failed: " + str(result.stderr)
        raise RuntimeError(msg)
    print("✅ Code generation complete")


def run_bootstrap_microservice(
    service_name: str,
    port: int | None,
    project_root: Path,
    add_dependencies_config: bool = False,
    layout: dict[str, Any] | None = None,
) -> int:
    """Bootstrap microservice. Returns 0 on success, 1 on error."""
    cfg = resolve_bootstrap_layout(layout)
    openapi_dir = cfg["openapi_dir"]
    suite = cfg["suite"]
    workspace_dir = cfg["workspace_dir"]
    docker_dir = cfg["docker_dir"]
    crate_name_prefix = cfg["crate_name_prefix"]

    if port is None:
        port = _get_port_from_registry(project_root, service_name, layout)
    if port is None:
        print(
            f"⚠️  No port for {service_name}. Run: rerp ports assign {service_name} --update-configs"
        )
        return 1

    spec_path = project_root / openapi_dir / suite / service_name / "openapi.yaml"
    crate_dir = project_root / workspace_dir / suite / service_name
    gen_dir = crate_dir / "gen"
    impl_dir = crate_dir / "impl"
    dockerfile_path = project_root / docker_dir / f"Dockerfile.{service_name}"
    config_path = impl_dir / "config" / "config.yaml"
    cargo_toml_path = project_root / workspace_dir / "Cargo.toml"
    tiltfile_path = project_root / cfg["tiltfile"]

    if not spec_path.exists():
        print(f"❌ OpenAPI spec not found: {spec_path}")
        return 1

    openapi_spec = load_yaml_spec(spec_path)
    binary_name = derive_binary_name(openapi_spec, service_name)
    print(f"🚀 Bootstrapping {service_name} (port {port}, binary {binary_name})")

    generate_code_with_brrtrouter(spec_path, gen_dir, project_root)

    gen_cargo = gen_dir / "Cargo.toml"
    if gen_cargo.exists():
        from brrtrouter_tooling.ci import run_fix_cargo_paths

        run_fix_cargo_paths(gen_cargo, project_root)
        _update_gen_cargo_toml(gen_cargo, service_name, crate_name_prefix)

    impl_dir.mkdir(parents=True, exist_ok=True)
    (impl_dir / "config").mkdir(parents=True, exist_ok=True)
    (impl_dir / "src" / "controllers").mkdir(parents=True, exist_ok=True)

    if not config_path.exists():
        create_config_yaml(config_path)

    if add_dependencies_config:
        deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"
        if not deps_config_path.exists():
            create_dependencies_config_toml(deps_config_path)

    if not (impl_dir / "Cargo.toml").exists():
        _generate_impl_with_brrtrouter(
            spec_path, impl_dir, service_name, project_root, crate_name_prefix
        )

    create_dockerfile(service_name, binary_name, port, dockerfile_path, workspace_dir, suite)
    update_workspace_cargo_toml(service_name, cargo_toml_path, suite)
    update_tiltfile(
        service_name,
        f"{service_name}/openapi.yaml",
        binary_name,
        port,
        tiltfile_path,
        workspace_dir,
        suite,
    )

    print(f"✅ Bootstrap complete for {service_name}. Next: tilt up")
    return 0
