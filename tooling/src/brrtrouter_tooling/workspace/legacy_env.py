"""Environment variables, with the pre-profile names still read for one release.

The generic names are ``BRRTROUTER_*``. Older names that carried a project's name are honoured with a
``DeprecationWarning`` naming the replacement; this module is the only place they appear (the name guard test in
``tests/test_no_project_names.py`` allows it).
"""

from __future__ import annotations

import functools
import os
import warnings
from collections.abc import Callable
from typing import Any

# generic name -> older names still read, in order
LEGACY_ENV: dict[str, tuple[str, ...]] = {
    "BRRTROUTER_PROJECT_ROOT": ("HAULIAGE_PROJECT_ROOT",),
    "BRRTROUTER_PORT_REGISTRY": ("HAULIAGE_PORT_REGISTRY", "RERP_PORT_REGISTRY"),
    "BRRTROUTER_TILT_PORT": ("HAULIAGE_TILT_PORT",),
    "BRRTROUTER_USE_CROSS": ("BRRT_USE_CROSS", "RERP_USE_CROSS", "Hauliage_USE_CROSS"),
}

# Package names with these prefixes predate profiles and are passed through unchanged by the build helpers.
LEGACY_PACKAGE_PREFIXES: tuple[str, ...] = ("rerp_",)


def env(name: str, default: str | None = None) -> str | None:
    """The value of ``name`` (a ``BRRTROUTER_*`` variable), else of its older name with a warning, else ``default``."""
    v = os.environ.get(name)
    if v:
        return v
    for old in LEGACY_ENV.get(name, ()):
        v = os.environ.get(old)
        if v:
            warnings.warn(
                f"{old} is deprecated; set {name} instead",
                DeprecationWarning,
                stacklevel=2,
            )
            return v
    return default


def env_flag(name: str) -> bool:
    """True when ``env(name)`` is ``"1"``."""
    return env(name) == "1"


def deprecated_alias(fn: Callable[..., Any], old_name: str) -> Callable[..., Any]:
    """``fn`` under an older name, warning on each call (kept for one release)."""

    @functools.wraps(fn)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        warnings.warn(
            f"{old_name} is deprecated; use {fn.__name__}", DeprecationWarning, stacklevel=2
        )
        return fn(*args, **kwargs)

    wrapper.__name__ = old_name
    return wrapper
