"""Port registry and validate/reconcile/fix-duplicates (RERP-style layout)."""

import importlib
import sys

__all__ = [
    "PortRegistry",
    "fix_duplicates",
    "reconcile",
    "validate",
]


def __getattr__(name: str):
    """Lazy-load registry and validate to avoid circular import with discovery."""
    if name == "PortRegistry":
        from brrtrouter_tooling.ports.registry import PortRegistry

        return PortRegistry
    if name in ("fix_duplicates", "reconcile", "validate"):
        mod = importlib.import_module("brrtrouter_tooling.ports.validate")
        obj = getattr(mod, name)
        # Cache so "from ports import validate" gets the function, not the submodule
        sys.modules[__name__].__dict__[name] = obj
        return obj
    msg = f"module {__name__!r} has no attribute {name!r}"
    raise AttributeError(msg)
