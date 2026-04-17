"""Tests for ci.fix_impl_dependencies (impl Cargo.toml clap/may + gen-driven deps)."""

from pathlib import Path


def test_adds_clap_may_when_workspace_impl(tmp_path: Path) -> None:
    from brrtrouter_tooling.ci.fix_impl_dependencies import update_impl_cargo_dependencies

    impl = tmp_path / "svc" / "impl"
    gen = tmp_path / "svc" / "gen"
    impl.mkdir(parents=True)
    gen.mkdir(parents=True)
    (gen / "Cargo.toml").write_text('[package]\nname = "x"\nversion = "0.1.0"\n')
    cargo = impl / "Cargo.toml"
    cargo.write_text(
        """[package]
name = "x_impl"
version = "0.1.0"

[dependencies]
x = { path = "../gen" }
brrtrouter = { workspace = true }
tikv-jemallocator = { workspace = true, optional = true }
"""
    )
    assert update_impl_cargo_dependencies(cargo)
    out = cargo.read_text()
    assert "clap = { workspace = true }" in out
    assert "may = { workspace = true }" in out


def test_adds_clap_may_when_non_workspace_versioned_tikv(tmp_path: Path) -> None:
    """Standalone impl (no [workspace]) uses versioned tikv-jemallocator; still needs clap/may."""
    from brrtrouter_tooling.ci.fix_impl_dependencies import update_impl_cargo_dependencies

    impl = tmp_path / "svc" / "impl"
    gen = tmp_path / "svc" / "gen"
    impl.mkdir(parents=True)
    gen.mkdir(parents=True)
    (gen / "Cargo.toml").write_text('[package]\nname = "x"\nversion = "0.1.0"\n')
    cargo = impl / "Cargo.toml"
    cargo.write_text(
        """[package]
name = "x_impl"
version = "0.1.0"

[dependencies]
x = { path = "../gen" }
brrtrouter = { path = "../BRRTRouter" }
tikv-jemallocator = { version = "0.6", features = ["profiling"], optional = true }
"""
    )
    assert update_impl_cargo_dependencies(cargo)
    out = cargo.read_text()
    assert 'clap = { version = "4.6", features = ["derive"] }' in out
    assert 'may = "0.3"' in out


def test_idempotent_after_clap_may(tmp_path: Path) -> None:
    from brrtrouter_tooling.ci.fix_impl_dependencies import update_impl_cargo_dependencies

    impl = tmp_path / "svc" / "impl"
    gen = tmp_path / "svc" / "gen"
    impl.mkdir(parents=True)
    gen.mkdir(parents=True)
    (gen / "Cargo.toml").write_text('[package]\nname = "x"\nversion = "0.1.0"\n')
    cargo = impl / "Cargo.toml"
    cargo.write_text(
        """[package]
name = "x_impl"
version = "0.1.0"

[dependencies]
x = { path = "../gen" }
brrtrouter = { workspace = true }
tikv-jemallocator = { workspace = true, optional = true }
clap = { workspace = true }
may = { workspace = true }
"""
    )
    assert not update_impl_cargo_dependencies(cargo)
