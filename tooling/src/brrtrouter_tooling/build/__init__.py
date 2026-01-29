"""Host-aware Rust build (cargo/cross/zigbuild, multi-arch). Consumed by RERP and other projects."""

from .host_aware import (
    ARCH_TARGETS,
    detect_host_architecture,
    run as run_host_aware,
    should_use_cross,
    should_use_zigbuild,
)
from .workspace_build import (
    build_package_with_options,
    build_workspace_with_options,
)

__all__ = [
    "ARCH_TARGETS",
    "build_package_with_options",
    "build_workspace_with_options",
    "detect_host_architecture",
    "run_host_aware",
    "should_use_cross",
    "should_use_zigbuild",
]
