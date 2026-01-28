"""Dependabot CLI commands."""

from brrtrouter_tooling.dependabot.automerge import process_dependabot_pr


def automerge() -> None:
    """Process and auto-merge Dependabot PRs."""
    process_dependabot_pr()
