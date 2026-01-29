"""Tests for brrtrouter_tooling.release.bump (_next_version and run)."""

import pytest

from brrtrouter_tooling.release.bump import _next_version


class TestNextVersionRc:
    """RC bump: full -> -rc.1, -rc.N -> -rc.N+1; non-rc prerelease raises."""

    def test_full_release_to_rc_one(self) -> None:
        assert _next_version("1.2.3", "rc") == "1.2.3-rc.1"
        assert _next_version("v1.2.3", "rc") == "1.2.3-rc.1"

    def test_rc_increments(self) -> None:
        assert _next_version("1.2.3-rc.1", "rc") == "1.2.3-rc.2"
        assert _next_version("1.2.3-rc.2", "rc") == "1.2.3-rc.3"
        assert _next_version("1.2.3-rc.99", "rc") == "1.2.3-rc.100"

    def test_rc_bump_with_non_rc_prerelease_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2.3-alpha", "rc")
        assert "rc bump only supports" in str(exc_info.value)
        assert "alpha" in str(exc_info.value)


class TestNextVersionReleasePromote:
    """Release/promote: -rc.N -> X.Y.Z; full release raises."""

    def test_rc_to_full_release(self) -> None:
        assert _next_version("1.2.3-rc.1", "release") == "1.2.3"
        assert _next_version("1.2.3-rc.1", "promote") == "1.2.3"
        assert _next_version("1.0.0-rc.5", "release") == "1.0.0"

    def test_full_release_promote_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2.3", "release")
        assert "Already a full release" in str(exc_info.value)
        with pytest.raises(SystemExit) as exc_info2:
            _next_version("1.2.3", "promote")
        assert "Already a full release" in str(exc_info2.value)


class TestNextVersionPatch:
    """Patch: X.Y.Z -> X.Y.(Z+1); prerelease + patch raises."""

    def test_patch_increments_z(self) -> None:
        assert _next_version("1.2.3", "patch") == "1.2.4"
        assert _next_version("0.1.0", "patch") == "0.1.1"
        assert _next_version("v2.10.99", "patch") == "2.10.100"

    def test_patch_on_prerelease_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2.3-rc.1", "patch")
        assert "Cannot patch bump prerelease" in str(exc_info.value)
        assert "release/promote or rc" in str(exc_info.value)


class TestNextVersionMinor:
    """Minor: X.Y.Z -> X.(Y+1).0; prerelease + minor raises."""

    def test_minor_increments_y_resets_z(self) -> None:
        assert _next_version("1.2.3", "minor") == "1.3.0"
        assert _next_version("0.1.0", "minor") == "0.2.0"
        assert _next_version("v3.0.5", "minor") == "3.1.0"

    def test_minor_on_prerelease_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2.3-rc.1", "minor")
        assert "Cannot minor bump prerelease" in str(exc_info.value)


class TestNextVersionMajor:
    """Major: X.Y.Z -> (X+1).0.0; prerelease + major raises."""

    def test_major_increments_x_resets_y_z(self) -> None:
        assert _next_version("1.2.3", "major") == "2.0.0"
        assert _next_version("0.1.0", "major") == "1.0.0"
        assert _next_version("v2.10.99", "major") == "3.0.0"

    def test_major_on_prerelease_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2.3-rc.1", "major")
        assert "Cannot major bump prerelease" in str(exc_info.value)


class TestNextVersionDefaultAndUnknown:
    """Default bump is patch; unknown bump raises."""

    def test_default_bump_is_patch(self) -> None:
        assert _next_version("1.2.3", "") == "1.2.4"
        assert _next_version("1.2.3", None) == "1.2.4"

    def test_unknown_bump_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2.3", "preminor")
        assert "Unknown bump" in str(exc_info.value)
        assert "patch, minor, major, rc, or release" in str(exc_info.value)


class TestNextVersionInvalid:
    """Invalid version string raises."""

    def test_invalid_version_raises(self) -> None:
        with pytest.raises(SystemExit) as exc_info:
            _next_version("1.2", "patch")
        assert "Invalid version" in str(exc_info.value)
        with pytest.raises(SystemExit) as exc_info2:
            _next_version("abc", "patch")
        assert "Invalid version" in str(exc_info2.value)
