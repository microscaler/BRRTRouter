//! Story 13.1 — doc truth & claim reconciliation fixtures.

#[test]
fn epic13_1_p1_openapi_gap_marks_ref_requestbodies_supported() {
    let gap = include_str!("../docs/OPENAPI_3.1.0_COMPLIANCE_GAP.md");
    assert!(
        gap.contains("1b. Shipped in Epic 12.3") && gap.contains("components.requestBodies `$ref`"),
        "OPENAPI gap must document 12.3 requestBodies support"
    );
    assert!(
        gap.contains("✅ Local resolved (12.3)") || gap.contains("✅ Local resolved"),
        "requestBodies/responses rows must not remain ❌-only"
    );
}

#[test]
fn epic13_1_p2_readme_links_epic_13_board() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md"),
        "README must link Epic 13 / framework board"
    );
    assert!(
        readme.contains("EPICS_CATALOG.md") || readme.contains("Epic 13"),
        "README must point at active epics catalog or Epic 13"
    );
}

#[test]
fn epic13_1_p3_epic_12_not_now_in_readme_roadmap() {
    let readme = include_str!("../README.md");
    assert!(
        !readme.contains("Now      Epic 12 Wave 4"),
        "README must not list Epic 12 Wave 4 as Now"
    );
    assert!(
        readme.contains("Epic 12 **done**") || readme.contains("Done     Epic 10–12"),
        "README should mark Epic 12 complete"
    );
}

#[test]
fn epic13_1_p4_jwks_preferred_over_oauth2_stub() {
    let sec = include_str!("../docs/SecurityAuthentication.md");
    assert!(
        sec.contains("JWT_AND_IDENTITY_BOUNDARY.md"),
        "security docs must link identity boundary"
    );
    assert!(
        sec.contains("Stub/dev") || sec.contains("not an authorization server"),
        "OAuth2Provider must be labeled non-production"
    );
}

#[test]
fn epic13_1_p5_catalog_lists_epic_13() {
    let cat = include_str!("../docs/EPICS/EPICS_CATALOG.md");
    assert!(cat.contains("Epic 13") || cat.contains("| 13 |"));
    assert!(cat.contains("epic-13-framework-completeness"));
}

#[test]
fn epic13_1_p6_multipart_mvp_or_streaming_pointer() {
    let mp = include_str!("../docs/multipart.md");
    assert!(
        mp.to_lowercase().contains("mvp") || mp.contains("13.4") || mp.contains("stream"),
        "multipart docs should describe MVP-A or point at streaming story"
    );
}

#[test]
fn epic13_1_n1_rate_limit_shipped_claim_matches_implementation() {
    let life = include_str!("../docs/RequestLifecycle.md");
    assert!(
        life.contains("**RateLimitMiddleware** | ✅ Shipped"),
        "RequestLifecycle must mark RateLimitMiddleware as shipped after 13.2"
    );
    assert!(
        !life.contains("**RateLimitMiddleware** | 🚧 **Not shipped**"),
        "stale Not shipped RateLimitMiddleware row forbidden"
    );
}

#[test]
fn epic13_1_n2_beginner_guide_claims_shipped_7807_after_13_3() {
    let guide = include_str!("../docs/marketing/BEGINNER_GUIDE.md");
    assert!(
        guide.contains("RFC 7807") && guide.contains("problem+json"),
        "BEGINNER_GUIDE must claim shipped RFC 7807 after 13.3"
    );
    assert!(
        !guide.contains("RFC 7807 `problem+json` is planned"),
        "stale planned caveat for 7807 forbidden"
    );
}

#[test]
fn epic13_1_n3_no_shipped_compression_middleware_claim() {
    let life = include_str!("../docs/RequestLifecycle.md");
    assert!(
        life.contains("CompressionMiddleware") && life.contains("Not shipped"),
        "CompressionMiddleware must be marked not shipped"
    );
}

#[test]
fn epic13_1_n4_gap_doc_not_wrong_on_requestbodies() {
    let gap = include_str!("../docs/OPENAPI_3.1.0_COMPLIANCE_GAP.md");
    // Stale absolute ❌ for requestBodies in components table superseded by §1b / ✅ rows.
    assert!(
        gap.contains("### 1b. Shipped in Epic 12.3"),
        "must have reconciliation section"
    );
    assert!(
        !gap.contains(
            "| **components.requestBodies** | `#/components/requestBodies/X` | ❌ Not resolved |"
        ),
        "stale ❌ Not resolved row for requestBodies forbidden"
    );
}

#[test]
fn epic13_1_n6_ws_not_in_progress_mvp() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("WebSocket **parked**") || readme.contains("Parked   WebSocket"),
        "WebSocket must remain parked"
    );
}
