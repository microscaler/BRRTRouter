"""Tests for brrtrouter_tooling.pre_commit.workspace_fmt."""

from pathlib import Path
from unittest.mock import MagicMock, patch


class TestRunWorkspaceFmt:
    def test_returns_0_when_no_changes_under_workspace(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.pre_commit import run_workspace_fmt

        with patch(
            "brrtrouter_tooling.pre_commit.workspace_fmt._run",
            side_effect=[
                MagicMock(returncode=0, stdout=""),  # git diff --name-only: no files
            ],
        ):
            code = run_workspace_fmt(tmp_path, workspace_dir="microservices")
        assert code == 0

    def test_returns_0_when_workspace_changed_and_fmt_succeeds_no_new_diff(
        self, tmp_path: Path
    ) -> None:
        from brrtrouter_tooling.pre_commit import run_workspace_fmt

        with patch(
            "brrtrouter_tooling.pre_commit.workspace_fmt._run",
            side_effect=[
                MagicMock(returncode=0, stdout="microservices/foo.rs\n"),  # changed
                MagicMock(returncode=0, stdout="", stderr=""),  # cargo fmt ok
                MagicMock(returncode=0, stdout="", stderr=""),  # git diff --exit-code ok
            ],
        ):
            code = run_workspace_fmt(tmp_path, workspace_dir="microservices")
        assert code == 0

    def test_returns_1_when_fmt_changes_files(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.pre_commit import run_workspace_fmt

        with patch(
            "brrtrouter_tooling.pre_commit.workspace_fmt._run",
            side_effect=[
                MagicMock(returncode=0, stdout="microservices/foo.rs\n"),
                MagicMock(returncode=0, stdout="", stderr=""),
                MagicMock(returncode=1, stdout="", stderr=""),  # git diff --exit-code: has diff
            ],
        ):
            code = run_workspace_fmt(tmp_path, workspace_dir="microservices")
        assert code == 1

    def test_returns_1_when_cargo_fmt_fails(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.pre_commit import run_workspace_fmt

        with patch(
            "brrtrouter_tooling.pre_commit.workspace_fmt._run",
            side_effect=[
                MagicMock(returncode=0, stdout="microservices/foo.rs\n"),
                MagicMock(returncode=1, stdout="", stderr="cargo failed"),
            ],
        ):
            code = run_workspace_fmt(tmp_path, workspace_dir="microservices")
        assert code == 1

    def test_uses_extra_check_dirs(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.pre_commit import run_workspace_fmt

        calls = []

        def record_run(cmd, cwd=None):
            calls.append(list(cmd))
            if "git diff --name-only" in " ".join(cmd):
                return MagicMock(returncode=0, stdout="microservices/x.rs\n")
            if cmd == ["cargo", "fmt"]:
                return MagicMock(returncode=0, stdout="", stderr="")
            if "git diff --exit-code" in " ".join(cmd):
                return MagicMock(returncode=0, stdout="", stderr="")
            return MagicMock(returncode=0, stdout="", stderr="")

        with patch("brrtrouter_tooling.pre_commit.workspace_fmt._run", side_effect=record_run):
            run_workspace_fmt(
                tmp_path,
                workspace_dir="microservices",
                extra_check_dirs=["microservices/", "entities/"],
            )
        exit_code_calls = [c for c in calls if c[:3] == ["git", "diff", "--exit-code"]]
        assert len(exit_code_calls) == 2
        assert "--" in exit_code_calls[0] and "microservices/" in exit_code_calls[0]
        assert "entities/" in exit_code_calls[1]
