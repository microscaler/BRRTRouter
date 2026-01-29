"""Fix impl crate Cargo.toml files to include dependencies used by gen crates."""

import re
from pathlib import Path


def update_impl_cargo_dependencies(impl_cargo_path: Path) -> bool:
    """Update impl Cargo.toml to include rust_decimal/rusty-money if gen crate uses them. Returns True if modified."""
    if not impl_cargo_path.exists():
        return False

    gen_dir = impl_cargo_path.parent.parent / "gen"
    if not gen_dir.exists():
        return False

    uses_decimal = False
    uses_money = False

    gen_cargo = gen_dir / "Cargo.toml"
    if gen_cargo.exists():
        gen_cargo_content = gen_cargo.read_text()
        uses_decimal = "rust_decimal" in gen_cargo_content
        uses_money = "rusty-money" in gen_cargo_content
    else:
        gen_src_dir = gen_dir / "src"
        if gen_src_dir.exists():
            for rust_file in gen_src_dir.rglob("*.rs"):
                try:
                    file_content = rust_file.read_text()
                    if "rust_decimal::Decimal" in file_content or "Decimal" in file_content:
                        uses_decimal = True
                    if "rusty_money::Money" in file_content or "Money<" in file_content:
                        uses_money = True
                    if uses_decimal and uses_money:
                        break
                except (OSError, UnicodeDecodeError):
                    continue

    content = impl_cargo_path.read_text()
    modified = False

    if uses_decimal and "rust_decimal" not in content:
        if "tikv-jemallocator" in content:
            content = re.sub(
                r"(tikv-jemallocator = \{[^\}]+\}\n)",
                r"\1rust_decimal = { workspace = true }\n",
                content,
                count=1,
            )
        else:
            content = re.sub(
                r"(\[dependencies\][^\[]+)(\n)",
                r"\1rust_decimal = { workspace = true }\2",
                content,
                count=1,
            )
        modified = True

    if uses_money and "rusty-money" not in content:
        if "rust_decimal" in content:
            content = re.sub(
                r"(rust_decimal = \{[^\}]+\}\n)",
                r"\1rusty-money = { workspace = true }\n",
                content,
                count=1,
            )
        elif "tikv-jemallocator" in content:
            content = re.sub(
                r"(tikv-jemallocator = \{[^\}]+\}\n)",
                r"\1rusty-money = { workspace = true }\n",
                content,
                count=1,
            )
        else:
            content = re.sub(
                r"(\[dependencies\][^\[]+)(\n)",
                r"\1rusty-money = { workspace = true }\2",
                content,
                count=1,
            )
        modified = True

    if modified:
        impl_cargo_path.write_text(content)
        return True
    return False


def fix_all_impl_dependencies(impl_base_dir: Path) -> int:
    """Fix all impl Cargo.toml under impl_base_dir (e.g. microservices/accounting). Returns 0."""
    if not impl_base_dir.exists():
        print(f"❌ Directory not found: {impl_base_dir}")
        return 1

    fixed_count = 0
    for service_dir in sorted(impl_base_dir.iterdir()):
        if not service_dir.is_dir():
            continue
        impl_cargo = service_dir / "impl" / "Cargo.toml"
        if impl_cargo.exists() and update_impl_cargo_dependencies(impl_cargo):
            print(f"✅ Updated {impl_cargo}")
            fixed_count += 1

    if fixed_count > 0:
        print(f"\n✅ Fixed {fixed_count} impl Cargo.toml file(s)")
    else:
        print("✅ All impl Cargo.toml files are up to date")
    return 0
