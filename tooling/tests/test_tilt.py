"""Tests for brrtrouter_tooling.tilt."""

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
            m.side_effect = ["/usr/bin/docker", None]  # docker ok, tilt missing
            with patch("subprocess.run", return_value=MagicMock(returncode=0)):
                assert run(tmp_path) == 1

    def test_returns_0_creates_dirs_and_volumes_mocked(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.setup import run

        dirs = [
            "docker/prometheus",
            "openapi/accounting",
            "microservices/accounting",
            "k8s/data",
        ]
        volumes = ["postgres_data"]
        with (
            patch("shutil.which", return_value="/usr/bin/docker"),
            patch("subprocess.run", return_value=MagicMock(returncode=0)),
        ):
            assert run(tmp_path, dirs=dirs, volumes=volumes) == 0
        for p in dirs:
            assert (tmp_path / p).is_dir()


class TestTeardown:
    def test_returns_0_with_mocked_subprocess(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.teardown import run

        with patch("subprocess.run", return_value=MagicMock(returncode=0)):
            assert (
                run(
                    tmp_path,
                    [],
                    remove_images=False,
                    remove_volumes=False,
                    system_prune=False,
                )
                == 0
            )

    def test_returns_0_with_remove_flags_mocked(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.teardown import run

        with patch("subprocess.run", return_value=MagicMock(returncode=0)):
            assert (
                run(
                    tmp_path,
                    [],
                    static_containers=["postgres-dev"],
                    volume_names=["postgres_data"],
                    remove_images=True,
                    remove_volumes=True,
                    system_prune=True,
                )
                == 0
            )

    def test_uses_container_and_image_fns(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.tilt.teardown import run

        def container_name_fn(s: str) -> str:
            return f"rerp-{s}-dev"

        def image_rmi_list_fn(s: str) -> list[str]:
            return [
                f"rerp-accounting-{s}:latest",
                f"localhost:5001/rerp-accounting-{s}:tilt",
            ]

        service_names = ["bff", "edi", "financial-reports"]
        with patch("subprocess.run", return_value=MagicMock(returncode=0)) as m:
            run(
                tmp_path,
                service_names,
                container_name_fn=container_name_fn,
                image_rmi_list_fn=image_rmi_list_fn,
                remove_images=True,
                remove_volumes=False,
                system_prune=False,
            )
        calls = [c[0][0] for c in m.call_args_list]
        assert ["docker", "stop", "rerp-bff-dev"] in [
            c[:3] for c in calls if c[:2] == ["docker", "stop"] and len(c) > 2
        ]
        assert ["docker", "stop", "rerp-edi-dev"] in [
            c[:3] for c in calls if c[:2] == ["docker", "stop"] and len(c) > 2
        ]
        rmi_calls = [c for c in calls if c[:2] == ["docker", "rmi"] and len(c) > 2]
        rmi_args = [c[2] for c in rmi_calls]
        assert "rerp-accounting-bff:latest" in rmi_args
        assert "localhost:5001/rerp-accounting-financial-reports:tilt" in rmi_args
