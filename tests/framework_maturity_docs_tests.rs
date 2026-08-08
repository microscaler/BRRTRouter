//! Story 12.1 — doc / status reconciliation fixtures (P*/N*).
#![allow(clippy::unwrap_used, clippy::expect_used)]

const README: &str = include_str!("../README.md");
const ROADMAP: &str = include_str!("../docs/ROADMAP.md");
const CONTRIBUTING: &str = include_str!("../CONTRIBUTING.md");
const EPICS_CATALOG: &str = include_str!("../docs/EPICS/EPICS_CATALOG.md");
const BUILD_BOARD: &str = include_str!("../docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md");
const STACK_SIZE: &str = include_str!("../docs/stack_size.md");
const OPENAPI_VERSION: &str = include_str!("../docs/OPENAPI_VERSION_SUPPORT.md");

/// P1 — README mentions radix / PathCursor.
#[test]
fn docs_12_1_positive_p1_readme_radix_pathcursor() {
    assert!(README.contains("radix") || README.contains("Radix"));
    assert!(README.contains("PathCursor"));
}

/// P2 — README does not claim regex matchers as the runtime router.
#[test]
fn docs_12_1_positive_p2_no_regex_runtime_claim() {
    assert!(
        !README.contains("Compiles OpenAPI paths into regex matchers"),
        "P2 stale regex routing claim"
    );
    assert!(
        !README.contains("| **Regex-Based Path Matching**"),
        "P2 regex feature row should be gone"
    );
}

/// P3 — Stack-size section points at docs/stack_size.md + vendor ext.
#[test]
fn docs_12_1_positive_p3_stack_size_docs() {
    assert!(README.contains("docs/stack_size.md"));
    assert!(
        README.contains("x-brrtrouter-stack-size")
            || STACK_SIZE.contains("x-brrtrouter-stack-size")
    );
    assert!(STACK_SIZE.contains("BRRTR_STACK_SIZE") || STACK_SIZE.contains("stack size"));
}

/// P4 — Epic 12 BUILD_BOARD linked from EPICS_CATALOG.
#[test]
fn docs_12_1_positive_p4_catalog_links_build_board() {
    assert!(
        EPICS_CATALOG.contains("FRAMEWORK_MATURITY/BUILD_BOARD.md"),
        "P4 EPICS_CATALOG must link Epic 12 BUILD_BOARD"
    );
    assert!(BUILD_BOARD.contains("12.1") && BUILD_BOARD.contains("12.2"));
}

/// P5 — WS marked parked / not falsely in progress.
#[test]
fn docs_12_1_positive_p5_websocket_parked() {
    assert!(README.contains("Parked") || README.contains("⏸"));
    assert!(
        README.to_ascii_lowercase().contains("websocket")
            && (README.contains("Parked") || README.contains("parked")),
        "P5 WebSocket must be marked parked"
    );
    assert!(BUILD_BOARD.contains("WebSocket"));
}

/// P6 — Typed panic recovery described accurately.
#[test]
fn docs_12_1_positive_p6_typed_panic_recovery() {
    assert!(README.contains("catch_unwind"));
    assert!(
        README.contains("typed handlers recover")
            || README.contains("Typed handlers")
            || README.contains("typed handlers"),
        "P6 typed panic recovery"
    );
    assert!(
        !README.contains("typed handlers do not"),
        "P6 must not claim typed handlers lack panic recovery"
    );
}

/// N1 — ROADMAP must not list CORS/metrics as unstarted planned work.
#[test]
fn docs_12_1_negative_n1_roadmap_not_stale_planned() {
    assert!(
        ROADMAP.contains("archive")
            || ROADMAP.contains("Archive")
            || ROADMAP.contains("historical"),
        "N1 ROADMAP should be archived / dated"
    );
    // Live “Planned” section must not claim CORS/metrics are still TODO.
    let before_archive = ROADMAP
        .split("Archive — May 2025")
        .next()
        .unwrap_or(ROADMAP);
    assert!(
        !before_archive.contains("- Middleware hooks for tracing, and CORS"),
        "N1 CORS must not appear as open planned work outside archive"
    );
    assert!(
        !before_archive.contains("- Prometheus-compatible metrics endpoint"),
        "N1 metrics must not appear as open planned work outside archive"
    );
}

/// N2 — CONTRIBUTING must not steer to WS as primary MVP gap.
#[test]
fn docs_12_1_negative_n2_contributing_not_ws_mvp() {
    assert!(CONTRIBUTING.contains("Epic 12") || CONTRIBUTING.contains("FRAMEWORK_MATURITY"));
    assert!(
        CONTRIBUTING.contains("Parked") || CONTRIBUTING.contains("parked"),
        "N2 must mark WebSocket parked"
    );
    // Must not list WebSocket as a primary 🚧 contribution bullet without parked context.
    assert!(
        !CONTRIBUTING.contains("- 🚧 WebSocket support\n"),
        "N2 must not list WebSocket as a primary open contribution area"
    );
}

/// N3 — No conflicting stack size 🚧 vs shipped override.
#[test]
fn docs_12_1_negative_n3_no_stack_size_wip_conflict() {
    assert!(
        !README.contains("| **handler coroutine stack size**                 | 🚧"),
        "N3 conflicting stack-size 🚧 row"
    );
}

/// N4 — Relative links in updated docs resolve on disk.
#[test]
fn docs_12_1_negative_n4_relative_links_exist() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "docs/stack_size.md",
        "docs/request_body_limits.md",
        "docs/webhook_delivery.md",
        "docs/OPENAPI_VERSION_SUPPORT.md",
        "docs/BUILDING_WITH_BRRTROUTER.md",
        "docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md",
        "docs/EPICS/EPICS_CATALOG.md",
    ] {
        assert!(root.join(rel).is_file(), "N4 missing {rel}");
    }
    assert!(!OPENAPI_VERSION.is_empty());
}

/// Public reference product is Sesame-IDAM (not private Hauliage / PW / immature RERP).
#[test]
fn docs_public_reference_is_sesame() {
    assert!(README.contains("sesame-idam") || README.contains("Sesame-IDAM"));
    assert!(README.contains("BUILDING_WITH_BRRTROUTER.md"));
    let building = include_str!("../docs/BUILDING_WITH_BRRTROUTER.md");
    assert!(building.contains("https://github.com/microscaler/sesame-idam"));
    assert!(building.contains("Hauliage") && building.contains("Private"));
    assert!(building.contains("RERP") && building.to_ascii_lowercase().contains("immature"));
}

/// N5 — Epic 10/11 not marked incomplete incorrectly on live ROADMAP.
#[test]
fn docs_12_1_negative_n5_epic_10_11_shipped() {
    assert!(
        ROADMAP.contains("Epic 10") && ROADMAP.contains("Epic 11"),
        "N5 should mention Epics 10/11 as shipped"
    );
    assert!(
        ROADMAP.contains("Shipped recently") || ROADMAP.contains("shipped"),
        "N5"
    );
}

/// N6 — Useful roadmap items not silently deleted (archive note present).
#[test]
fn docs_12_1_negative_n6_archive_note() {
    assert!(
        ROADMAP.contains("historical")
            || ROADMAP.contains("archaeology")
            || ROADMAP.contains("Archive"),
        "N6 archive note required"
    );
}
