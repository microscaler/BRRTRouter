"""Tests for hauliage docker build-image-simple (delegates to brrtrouter_tooling.docker.build_image_simple)."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

pytest.importorskip("brrtrouter_tooling")


class TestBuildImageSimple:
    def test_hash_missing_returns_1(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "art").write_bytes(b"x")
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        assert (
            run(
                "img",
                Path("missing.sha256"),
                Path("art"),
                tmp_path,
                dockerfile=Path("Dockerfile"),
            )
            == 1
        )

    def test_artifact_missing_returns_1(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        assert (
            run(
                "img",
                Path("h.sha256"),
                Path("missing"),
                tmp_path,
                dockerfile=Path("Dockerfile"),
            )
            == 1
        )

    def test_dockerfile_missing_returns_1(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "art").write_bytes(b"x")
        assert (
            run(
                "img",
                Path("h.sha256"),
                Path("art"),
                tmp_path,
                dockerfile=Path("missing"),
            )
            == 1
        )

    def test_docker_build_fails_returns_1(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "art").write_bytes(b"x")
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        with patch("brrtrouter_tooling.docker.build_image_simple.subprocess.run") as m:
            m.return_value = MagicMock(returncode=1)
            assert (
                run(
                    "img",
                    Path("h.sha256"),
                    Path("art"),
                    tmp_path,
                    dockerfile=Path("Dockerfile"),
                )
                == 1
            )
            m.assert_called_once()
            assert m.call_args[0][0][:2] == ["docker", "build"]
            assert "--no-cache" not in m.call_args[0][0]

    def test_no_cache_adds_flag(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "art").write_bytes(b"x")
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        with patch("brrtrouter_tooling.docker.build_image_simple.subprocess.run") as m:
            m.return_value = MagicMock(returncode=0)
            assert (
                run(
                    "img",
                    Path("h.sha256"),
                    Path("art"),
                    tmp_path,
                    dockerfile=Path("Dockerfile"),
                    no_cache=True,
                )
                == 0
            )
            assert "--no-cache" in m.call_args_list[0][0][0]

    def test_docker_build_ok_push_ok_returns_0(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "art").write_bytes(b"x")
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        with patch("brrtrouter_tooling.docker.build_image_simple.subprocess.run") as m:
            m.return_value = MagicMock(returncode=0)
            assert (
                run(
                    "img",
                    Path("h.sha256"),
                    Path("art"),
                    tmp_path,
                    dockerfile=Path("Dockerfile"),
                )
                == 0
            )
            # Build to a local tag, retag to the registry name, then push (f654ea8).
            assert m.call_count >= 3
            build, tag, push = (c[0][0] for c in m.call_args_list[:3])
            assert build[:2] == ["docker", "build"]
            assert "--no-cache" not in build
            assert build[build.index("-t") + 1] == "brrtr-local/img:tilt"
            assert tag == ["docker", "tag", "brrtr-local/img:tilt", "img:tilt"]
            assert push == ["docker", "push", "img:tilt"]

    def test_docker_build_ok_push_fail_kind_ok_returns_0(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "art").write_bytes(b"x")
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        mod = "brrtrouter_tooling.docker.build_image_simple"
        # Every push route (docker, skopeo, crane, buildx) fails: _push_remote returns False.
        with (
            patch(f"{mod}.subprocess.run") as m,
            patch(f"{mod}._push_remote", return_value=False) as push,
        ):
            m.side_effect = [
                MagicMock(returncode=0),  # docker build
                MagicMock(returncode=0),  # docker tag
                MagicMock(returncode=0),  # kind load
            ]
            assert (
                run(
                    "img",
                    Path("h.sha256"),
                    Path("art"),
                    tmp_path,
                    dockerfile=Path("Dockerfile"),
                )
                == 0
            )
            assert m.call_count == 3
            push.assert_called_once()
            assert push.call_args[0][1] == "img:tilt"
            assert m.call_args_list[2][0][0][:3] == ["kind", "load", "docker-image"]

    def test_prune_dangling_after_calls_image_prune(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_image_simple import run

        (tmp_path / "h.sha256").write_text("a" * 64)
        (tmp_path / "art").write_bytes(b"x")
        (tmp_path / "Dockerfile").write_text("FROM alpine\n")
        with (
            patch("brrtrouter_tooling.docker.cleanup.prune_dangling_images") as prune,
            patch("brrtrouter_tooling.docker.build_image_simple.subprocess.run") as m,
        ):
            m.return_value = MagicMock(returncode=0)
            assert (
                run(
                    "img",
                    Path("h.sha256"),
                    Path("art"),
                    tmp_path,
                    dockerfile=Path("Dockerfile"),
                    prune_dangling_after=True,
                )
                == 0
            )
            prune.assert_called_once()
