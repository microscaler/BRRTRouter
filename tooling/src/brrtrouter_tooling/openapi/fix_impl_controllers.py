"""Fix impl controllers to use Decimal instead of f64 literals."""

import re
from pathlib import Path
from re import Match


def convert_f64_to_decimal(match: Match[str]) -> str:
    """Convert f64 literal to Decimal::new() call."""
    value = match.group(1)

    try:
        float_val = float(value)
    except ValueError:
        return match.group(0)

    if float_val.is_integer():
        int_val = int(float_val)
        return f"rust_decimal::Decimal::new({int_val}, 0)"

    str_val = str(float_val)
    if "." in str_val:
        parts = str_val.split(".")
        integer_part = parts[0]
        decimal_part = parts[1].rstrip("0")
        if not decimal_part:
            return f"rust_decimal::Decimal::new({int(float_val)}, 0)"
        mantissa_str = integer_part + decimal_part
        scale = len(decimal_part)
        mantissa = int(mantissa_str)
        return f"rust_decimal::Decimal::new({mantissa}, {scale})"

    return match.group(0)


def fix_impl_controller(file_path: Path) -> tuple[int, bool]:
    """Fix f64 literals in impl controller file.

    Returns (number of fixes, whether file was changed).
    """
    try:
        content = file_path.read_text()
    except (OSError, UnicodeDecodeError) as e:
        print(f"❌ Failed to read {file_path}: {e}")
        return (0, False)

    original_content = content

    pattern = r"Some\((-?\d+\.?\d*)\)"

    def replace_func(match: Match[str]) -> str:
        value = match.group(1)
        try:
            float_val = float(value)
            if "." in value or abs(float_val) >= 1000:
                decimal_expr = convert_f64_to_decimal(match)
                return f"Some({decimal_expr})"
        except ValueError:
            pass
        return match.group(0)

    content = re.sub(pattern, replace_func, content)

    pattern2 = r":\s+(\d+\.\d+),"

    def replace_func2(match: Match[str]) -> str:
        value = match.group(1)
        try:
            float(value)
            if "." in value:
                decimal_expr = convert_f64_to_decimal(match)
                return f": {decimal_expr},"
        except ValueError:
            pass
        return match.group(0)

    content = re.sub(pattern2, replace_func2, content)

    if content != original_content:
        file_path.write_text(content)
        fixes = len(re.findall(r"rust_decimal::Decimal::new", content)) - len(
            re.findall(r"rust_decimal::Decimal::new", original_content)
        )
        return (fixes, True)

    return (0, False)


def fix_impl_controllers_dir(impl_base_dir: Path) -> list[tuple[Path, int]]:
    """Fix all impl controller .rs files under impl_base_dir (e.g. microservices/accounting).

    Finds */impl/src/controllers/*.rs. Returns [(path, fixes), ...] for changed files.
    """
    if not impl_base_dir.exists():
        return []
    files_fixed: list[tuple[Path, int]] = []
    for controller_file in sorted(impl_base_dir.rglob("impl/src/controllers/*.rs")):
        if not controller_file.is_file():
            continue
        fixes, changed = fix_impl_controller(controller_file)
        if changed:
            files_fixed.append((controller_file, fixes))
    return files_fixed
