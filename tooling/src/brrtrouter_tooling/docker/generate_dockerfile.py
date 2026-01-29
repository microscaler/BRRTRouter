"""Generate service-specific Dockerfile from docker/microservices/Dockerfile.template."""

from __future__ import annotations

import sys
from pathlib import Path


def generate_dockerfile(
    system: str,
    module: str,
    port: int = 8000,
    project_root: Path | None = None,
    template_path: Path | None = None,
    output_path: Path | None = None,
    binary_name_pattern: str = "rerp_{system}_{module}_impl",
) -> Path:
    """
    Generate a Dockerfile for a specific service from the template.
    Writes to docker/microservices/Dockerfile.{system}_{module} unless output_path is set.
    binary_name_pattern may use {system} and {module} (with - replaced by _ for module).
    Returns the output path.
    """
    root = Path(project_root) if project_root is not None else Path.cwd()
    tpl = template_path or (root / "docker" / "microservices" / "Dockerfile.template")
    out = output_path or (root / "docker" / "microservices" / f"Dockerfile.{system}_{module}")

    if not tpl.exists():
        msg = f"Template not found: {tpl}"
        raise FileNotFoundError(msg)

    binary_name = binary_name_pattern.format(system=system, module=module.replace("-", "_"))
    content = tpl.read_text()
    content = content.replace("{{service_name}}", f"{system}-{module}")
    content = content.replace("{{binary_name}}", binary_name)
    content = content.replace("{{system}}", system)
    content = content.replace("{{module}}", module)
    content = content.replace("{{port}}", str(port))

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(content)
    print(f"✅ Generated: {out}")
    return out


def run(
    system: str,
    module: str,
    port: int = 8000,
    project_root: Path | None = None,
    binary_name_pattern: str = "rerp_{system}_{module}_impl",
) -> int:
    """CLI entry: generate Dockerfile. Returns 0 on success, 1 on error."""
    try:
        generate_dockerfile(
            system,
            module,
            port=port,
            project_root=project_root,
            binary_name_pattern=binary_name_pattern,
        )
        return 0
    except FileNotFoundError as e:
        print(f"❌ {e}", file=sys.stderr)
        return 1
