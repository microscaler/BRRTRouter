"""BFF OpenAPI generator: merge sub-service specs with proxy extensions and components/security."""

from brrtrouter_tooling.bff.config import load_suite_config
from brrtrouter_tooling.bff.discovery import (
    discover_sub_services,
    list_systems_with_sub_services,
)
from brrtrouter_tooling.bff.generate import generate_bff_spec
from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec

__all__ = [
    "discover_sub_services",
    "generate_bff_spec",
    "generate_system_bff_spec",
    "list_systems_with_sub_services",
    "load_suite_config",
]
