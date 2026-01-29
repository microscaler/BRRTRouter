"""Pre-commit helpers: run cargo fmt when a workspace dir has changed."""

from brrtrouter_tooling.pre_commit.workspace_fmt import run_workspace_fmt

__all__ = ["run_workspace_fmt"]
