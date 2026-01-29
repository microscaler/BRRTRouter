"""BFF OpenAPI generator: merge sub-service specs with proxy extensions and components/security."""

from brrtrouter_tooling.bff.config import load_suite_config
from brrtrouter_tooling.bff.generate import generate_bff_spec

__all__ = [
    "generate_bff_spec",
    "load_suite_config",
]
