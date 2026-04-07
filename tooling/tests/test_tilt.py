"""Tests for brrtrouter_tooling.tilt."""

import os
from pathlib import Path
from unittest.mock import MagicMock, patch


class TestSetupKindRegistry:
    def test_returns_0_when_network_and_registry_ok(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup_kind_registry import run

        with patch("subprocess.run") as m:
            m.side_effect = [
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="true"),
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="{}"),
                MagicMock(returncode=0),
                MagicMock(returncode=0),
            ]
            assert run(tmp_path) == 0

    def test_returns_1_when_kind_network_missing(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup_kind_registry import run

        with patch("subprocess.run") as m:
            m.side_effect = [
                MagicMock(returncode=0),
                MagicMock(returncode=0, stdout="true"),
                MagicMock(returncode=1),
            ]
            assert run(tmp_path) == 1


class TestSetupPersistentVolumes:
    def test_returns_0_when_skip_env_set(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup_persistent_volumes import run

        with patch.dict(os.environ, {"BRRTROUTER_SKIP_SETUP_PERSISTENT_VOLUMES": "1"}):
            assert run(tmp_path) == 0

    def test_returns_1_when_kubectl_not_installed(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup_persistent_volumes import run

        with patch("shutil.which", return_value=None):
            assert run(tmp_path) == 1

    def test_returns_1_when_kubectl_not_connected(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup_persistent_volumes import run

        with (
            patch("shutil.which", return_value="/usr/bin/kubectl"),
            patch("subprocess.run") as m,
        ):
            m.return_value = MagicMock(returncode=1)
            assert run(tmp_path) == 1

    def test_returns_0_when_no_pv_files(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup_persistent_volumes import run

        with (
            patch("shutil.which", return_value="/usr/bin/kubectl"),
            patch("subprocess.run") as m,
        ):
            m.return_value = MagicMock(returncode=0, stdout="")
            assert run(tmp_path) == 0


class TestLogs:
    def test_returns_1_when_tilt_not_in_path(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.logs import run

        with patch("shutil.which", return_value=None):
            assert run("general-ledger", tmp_path) == 1

    def test_returns_1_when_tilt_get_fails(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.logs import run

        with (
            patch("shutil.which", return_value="/usr/bin/tilt"),
            patch("subprocess.run") as m,
        ):
            m.side_effect = [MagicMock(returncode=1)]
            assert run("general-ledger", tmp_path) == 1

    def test_returns_1_when_component_not_found(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.logs import run

        with (
            patch("shutil.which", return_value="/usr/bin/tilt"),
            patch("subprocess.run") as m,
        ):
            m.side_effect = [
                MagicMock(returncode=0, stdout='[{"name":"other"}]'),
            ]
            assert run("general-ledger", tmp_path) == 1

    def test_returns_tilt_logs_exit_code_when_ok(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.logs import run

        with (
            patch("shutil.which", return_value="/usr/bin/tilt"),
            patch("subprocess.run") as m,
        ):
            m.side_effect = [
                MagicMock(returncode=0, stdout='[{"name":"general-ledger"}]'),
                MagicMock(returncode=0),
            ]
            assert run("general-ledger", tmp_path) == 0
            assert m.call_count == 2
            assert m.call_args_list[1][0][0][:2] == ["tilt", "logs"]


class TestSetup:
    def test_returns_1_when_docker_not_in_path(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup import run

        with patch("shutil.which", return_value=None):
            assert run(tmp_path) == 1

    def test_returns_1_when_tilt_not_in_path(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup import run

        with patch("shutil.which") as m:
            # Four volume iterations + final docker/tilt check in the dependency loop
            m.side_effect = ["/usr/bin/docker"] * 5 + [None]
            with patch("subprocess.run", return_value=MagicMock(returncode=0)):
                assert run(tmp_path) == 1

    def test_returns_0_creates_dirs_and_volumes_mocked(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup import run

        with (
            patch("shutil.which", return_value="/usr/bin/docker"),
            patch("subprocess.run", return_value=MagicMock(returncode=0)),
        ):
            assert run(tmp_path) == 0
        assert (tmp_path / "microservices").is_dir()
        assert (tmp_path / "k8s/data").is_dir()


class TestTeardown:
    def test_returns_0_with_mocked_subprocess(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.teardown import run

        with (
            patch("subprocess.run", return_value=MagicMock(returncode=0)),
            patch("brrtrouter_tooling.tilt.teardown.tilt_service_names", return_value=[]),
        ):
            assert run(tmp_path, remove_images=False, remove_volumes=False, system_prune=False) == 0

    def test_returns_0_with_remove_flags_mocked(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.teardown import run

        with (
            patch("subprocess.run", return_value=MagicMock(returncode=0)),
            patch(
                "brrtrouter_tooling.tilt.teardown.tilt_service_names",
                return_value=["bff"],
            ),
        ):
            assert (
                run(
                    tmp_path,
                    remove_images=True,
                    remove_volumes=True,
                    system_prune=True,
                )
                == 0
            )

    def test_remove_images_calls_rmi_for_services(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.teardown import run

        with (
            patch("subprocess.run", return_value=MagicMock(returncode=0)) as m,
            patch(
                "brrtrouter_tooling.tilt.teardown.tilt_service_names",
                return_value=["bff", "edi"],
            ),
        ):
            run(tmp_path, remove_images=True, remove_volumes=False, system_prune=False)
        calls = [c[0][0] for c in m.call_args_list]
        rmi_args = [c[2] for c in calls if c[:2] == ["docker", "rmi"] and len(c) > 2]
        assert any("bff" in a for a in rmi_args)
        assert any("edi" in a for a in rmi_args)
