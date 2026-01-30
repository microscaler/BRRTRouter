"""Code generation: call brrtrouter-gen (generate, generate-stubs); regenerate suite/service with default paths."""

from brrtrouter_tooling.gen.brrtrouter import (
    call_brrtrouter_generate,
    call_brrtrouter_generate_stubs,
    find_brrtrouter,
)
from brrtrouter_tooling.gen.regenerate import (
    regenerate_service,
    regenerate_suite_services,
    run_gen_if_missing_for_suite,
)

__all__ = [
    "call_brrtrouter_generate",
    "call_brrtrouter_generate_stubs",
    "find_brrtrouter",
    "regenerate_service",
    "regenerate_suite_services",
    "run_gen_if_missing_for_suite",
]
