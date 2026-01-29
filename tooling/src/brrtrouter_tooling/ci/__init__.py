"""CI automation: patch Cargo for BRRTRouter/lifeguard from git; fix path deps; get-latest-tag; is-tag; validate versions."""

from brrtrouter_tooling.helpers import compare_versions, find_cargo_tomls

from .fix_cargo_paths import fix_cargo_toml
from .fix_cargo_paths import run as run_fix_cargo_paths
from .get_latest_tag import get_latest_tag
from .get_latest_tag import run as run_get_latest_tag
from .is_tag import run as run_is_tag
from .patch_brrtrouter import (
    find_matches,
    patch_file,
    run_cargo_update,
)
from .patch_brrtrouter import (
    run as run_patch_brrtrouter,
)
from .validate_version import (
    run as run_validate_version,
)
from .validate_version import (
    run_validate_version_cli,
    validate_version,
)

__all__ = [
    "compare_versions",
    "find_cargo_tomls",
    "find_matches",
    "fix_cargo_toml",
    "get_latest_tag",
    "patch_file",
    "run_cargo_update",
    "run_fix_cargo_paths",
    "run_get_latest_tag",
    "run_is_tag",
    "run_patch_brrtrouter",
    "run_validate_version",
    "run_validate_version_cli",
    "validate_version",
]
