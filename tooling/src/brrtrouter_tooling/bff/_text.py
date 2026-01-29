"""Shared text helpers for BFF (schema naming, etc.)."""


def _to_pascal_case(name: str) -> str:
    """Convert kebab-case to PascalCase (e.g. my-service -> MyService)."""
    return "".join(word.capitalize() for word in name.split("-"))
