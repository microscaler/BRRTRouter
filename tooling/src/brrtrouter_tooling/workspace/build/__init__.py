"""Build: host-aware cargo/cross for workspace and services."""

from .host_aware import (
    ARCH_TARGETS,
)
from .host_aware import (
    run as run_host_aware,
)

__all__ = ["ARCH_TARGETS", "run_host_aware"]
