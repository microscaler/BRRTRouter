"""Generic tooling for **any** repository that uses BRRTRouter (not microscaler-specific).

This package holds optional helpers that assume a *workspace* layout (OpenAPI specs,
``microservices/``, Helm, Tilt, port registry, etc.). Core ``brrtrouter`` CLI commands
under ``brrtrouter client`` stay in ``brrtrouter_tooling.cli``; the ``workspace`` CLI
(``hauliage`` / project-specific entry points) layers argparse and discovery tweaks such
as flattened ``openapi/<service>/`` trees.

Configure paths with standard env vars (e.g. ``BRRTROUTER_ROOT``, ``BRRTROUTER_VENV``,
``HAULIAGE_PROJECT_ROOT`` or a future ``BRRTRouter_PROJECT_ROOT``) so the same code
works outside the Microscaler monorepo.
"""

__version__ = "0.1.0"
