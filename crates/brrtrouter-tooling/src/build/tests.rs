use crate::build::{
    detect_host_architecture, get_linker_env, should_use_cross, should_use_zigbuild, ARCH_TARGETS,
};

// ── ARCH_TARGETS tests ────────────────────────────────────────────────

#[test]
fn test_arch_targets_has_three_entries() {
    assert_eq!(ARCH_TARGETS.len(), 3);
    assert_eq!(ARCH_TARGETS[0], ("amd64", "x86_64-unknown-linux-musl"));
    assert_eq!(ARCH_TARGETS[1], ("arm64", "aarch64-unknown-linux-musl"));
    assert_eq!(ARCH_TARGETS[2], ("arm7", "armv7-unknown-linux-musleabihf"));
}

// ── detect_host_architecture tests ────────────────────────────────────

#[test]
fn test_detect_host_architecture_x86_64_env() {
    std::env::set_var("CARGO_TARGET_ARCH", "x86_64");
    assert_eq!(detect_host_architecture(), "amd64");
    std::env::remove_var("CARGO_TARGET_ARCH");
}

#[test]
fn test_detect_host_architecture_amd64_env() {
    std::env::set_var("CARGO_TARGET_ARCH", "amd64");
    assert_eq!(detect_host_architecture(), "amd64");
    std::env::remove_var("CARGO_TARGET_ARCH");
}

#[test]
fn test_detect_host_architecture_aarch64_env() {
    std::env::set_var("CARGO_TARGET_ARCH", "aarch64");
    assert_eq!(detect_host_architecture(), "arm64");
    std::env::remove_var("CARGO_TARGET_ARCH");
}

#[test]
fn test_detect_host_architecture_arm64_env() {
    std::env::set_var("CARGO_TARGET_ARCH", "arm64");
    assert_eq!(detect_host_architecture(), "arm64");
    std::env::remove_var("CARGO_TARGET_ARCH");
}

#[test]
fn test_detect_host_architecture_case_insensitive() {
    std::env::set_var("CARGO_TARGET_ARCH", "X86_64");
    assert_eq!(detect_host_architecture(), "amd64");
    std::env::remove_var("CARGO_TARGET_ARCH");

    std::env::set_var("CARGO_TARGET_ARCH", "AARCH64");
    assert_eq!(detect_host_architecture(), "arm64");
    std::env::remove_var("CARGO_TARGET_ARCH");
}

#[test]
fn test_detect_host_architecture_fallback_to_target() {
    std::env::remove_var("CARGO_TARGET_ARCH");
    std::env::set_var("TARGET", "aarch64");
    assert_eq!(detect_host_architecture(), "arm64");
    std::env::remove_var("TARGET");
}

#[test]
fn test_detect_host_architecture_fallback_default() {
    std::env::remove_var("CARGO_TARGET_ARCH");
    std::env::remove_var("TARGET");
    assert_eq!(detect_host_architecture(), "amd64");
}

// ── should_use_cross tests ────────────────────────────────────────────

#[test]
fn test_should_use_cross_true() {
    std::env::set_var("Hauliage_USE_CROSS", "1");
    assert!(should_use_cross());
    std::env::remove_var("Hauliage_USE_CROSS");
}

#[test]
fn test_should_use_cross_false_when_unset() {
    std::env::remove_var("Hauliage_USE_CROSS");
    assert!(!should_use_cross());
}

#[test]
fn test_should_use_cross_false_for_other_value() {
    std::env::set_var("Hauliage_USE_CROSS", "0");
    assert!(!should_use_cross());
    std::env::remove_var("Hauliage_USE_CROSS");
}

// ── should_use_zigbuild tests ─────────────────────────────────────────

#[test]
fn test_should_use_zigbuild_true_when_macos() {
    std::env::remove_var("TARGET_OS");
    std::env::set_var("TARGET_OS", "macos");
    assert!(should_use_zigbuild());
    std::env::remove_var("TARGET_OS");
}

#[test]
fn test_should_use_zigbuild_false_for_linux() {
    std::env::set_var("TARGET_OS", "linux");
    assert!(!should_use_zigbuild());
    std::env::remove_var("TARGET_OS");
}

#[test]
fn test_should_use_zigbuild_false_when_unset() {
    std::env::remove_var("TARGET_OS");
    assert!(!should_use_zigbuild());
}

// ── get_linker_env tests ──────────────────────────────────────────────

#[test]
fn test_get_linker_env_x86_64() {
    let env = get_linker_env("x86_64-unknown-linux-musl");
    assert_eq!(env.len(), 2);
    assert_eq!(env[0], ("CC_x86_64_unknown_linux_musl", "musl-gcc"));
    assert_eq!(
        env[1],
        ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER", "musl-gcc")
    );
}

#[test]
fn test_get_linker_env_aarch64() {
    let env = get_linker_env("aarch64-unknown-linux-musl");
    assert_eq!(env.len(), 2);
    assert_eq!(
        env[0],
        ("CC_aarch64_unknown_linux_musl", "aarch64-linux-musl-gcc")
    );
    assert_eq!(
        env[1],
        (
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER",
            "aarch64-linux-musl-gcc"
        )
    );
}

#[test]
fn test_get_linker_env_armv7() {
    let env = get_linker_env("armv7-unknown-linux-musleabihf");
    assert_eq!(env.len(), 2);
    assert_eq!(
        env[0],
        (
            "CC_armv7_unknown_linux_musleabihf",
            "arm-linux-musleabihf-gcc"
        )
    );
    assert_eq!(
        env[1],
        (
            "CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER",
            "arm-linux-musleabihf-gcc"
        )
    );
}

#[test]
fn test_get_linker_env_unknown_target_empty() {
    let env = get_linker_env("unknown-target-foo");
    assert!(env.is_empty());
}

#[test]
fn test_get_linker_env_all_targets_have_linker_env() {
    // Every ARCH_TARGETS triple should have a non-empty linker env
    for &(_, triple) in ARCH_TARGETS {
        let env = get_linker_env(triple);
        assert!(
            !env.is_empty(),
            "linker env for {} should not be empty",
            triple
        );
    }
}
