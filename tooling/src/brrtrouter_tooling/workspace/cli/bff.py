"""`hauliage bff` subcommands: generate-system."""

from pathlib import Path

from brrtrouter_tooling.workspace.bff import (
    discover_sub_services,
    generate_system_bff_spec,
    list_systems_with_sub_services,
)


def run_bff(args, project_root: Path) -> None:
    if getattr(args, "bff_cmd", None) == "generate-system":
        _run_generate_system(project_root, args)
        return
    msg = "bff: missing or unknown subcommand"
    raise ValueError(msg)


def _run_generate_system(project_root: Path, args) -> None:
    raw_dir = getattr(args, "openapi_dir", None)
    openapi_dir = (
        Path(raw_dir).expanduser().resolve() if raw_dir else (project_root / "openapi").resolve()
    )
    system = getattr(args, "system", None)
    output = getattr(args, "output", None)

    if system:
        out_path = Path(output).expanduser().resolve() if output else None
        nested_config = openapi_dir / system / "bff-suite-config.yaml"
        flat_config = openapi_dir / "bff-suite-config.yaml"
        has_suite_config = nested_config.is_file() or flat_config.is_file()
        subs = discover_sub_services(openapi_dir, system)
        if not subs and not has_suite_config:
            print(f"⚠️  No sub-services found for {system}")
            return
        n = len(subs) if subs else "(suite-config)"
        print(f"🔄 Generating {system} system BFF OpenAPI specification ({n} sub-services)...")
        generate_system_bff_spec(openapi_dir, system, output_path=out_path)
        if nested_config.is_file():
            out = (
                Path(output).expanduser().resolve()
                if output
                else (openapi_dir / system / "openapi_bff.yaml")
            )
        elif flat_config.is_file():
            out = (
                Path(output).expanduser().resolve()
                if output
                else (openapi_dir / "openapi_bff.yaml")
            )
        else:
            out = (
                Path(output).expanduser().resolve()
                if output
                else (openapi_dir / system / "openapi.yaml")
            )
        print(f"✅ Generated {system} BFF spec: {out}")
    else:
        systems = list_systems_with_sub_services(openapi_dir)
        print(
            f"🔄 Generating system BFF specs for all systems ({len(systems)} with sub-services)..."
        )
        for s in systems:
            generate_system_bff_spec(openapi_dir, s, output_path=None)
            nested = openapi_dir / s / "openapi_bff.yaml"
            flat = openapi_dir / "openapi_bff.yaml"
            if nested.is_file():
                print(f"✅ {s} → {nested}")
            elif flat.is_file():
                print(f"✅ {s} → {flat}")
            else:
                print(f"✅ {s} → {openapi_dir / s / 'openapi.yaml'}")
