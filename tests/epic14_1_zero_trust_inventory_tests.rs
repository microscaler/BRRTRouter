//! Story 14.1 — zero-trust inventory & threat model fixtures.

#[test]
fn epic14_1_p1_matrix_present() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    assert!(inv.contains("## Capability matrix"));
    assert!(inv.contains("X.509 SVID"));
    assert!(inv.contains("Federation"));
}

#[test]
fn epic14_1_p2_consumer_not_issuer() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    let boundary = include_str!("../docs/JWT_AND_IDENTITY_BOUNDARY.md");
    assert!(
        inv.contains("consumer") && inv.contains("not an issuer"),
        "inventory must state consumer-not-issuer"
    );
    assert!(
        boundary.contains("does **not** reproduce an identity provider")
            || boundary.contains("consumer and enforcer"),
        "boundary doc required"
    );
}

#[test]
fn epic14_1_p3_stories_cross_linked() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    for s in ["14.2", "14.3", "14.4", "14.5", "14.6", "14.7", "14.8"] {
        assert!(inv.contains(s), "inventory must link story {s}");
    }
}

#[test]
fn epic14_1_p4_threat_model_section() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    assert!(inv.contains("## Threat model"));
    assert!(inv.contains("Spoofed") || inv.contains("spoof"));
}

#[test]
fn epic14_1_p5_may_minihttp_tls_notes() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    assert!(inv.contains("may_minihttp"));
}

#[test]
fn epic14_1_n1_no_false_mtls_shipped() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    assert!(
        inv.contains("X.509 SVID parse") && inv.contains("❌"),
        "X.509 must still show not shipped"
    );
}

#[test]
fn epic14_1_n2_no_issuer_claim() {
    let boundary = include_str!("../docs/JWT_AND_IDENTITY_BOUNDARY.md");
    assert!(
        boundary.contains("Out of scope") && boundary.contains("Issue access"),
        "must list token issuance as out of scope"
    );
}

#[test]
fn epic14_1_n6_revoke_is_external_hook() {
    let inv = include_str!("../docs/SPIFFE_ZERO_TRUST_INVENTORY.md");
    assert!(
        inv.contains("external") && inv.contains("jti"),
        "revocation must be external checker, not IdP DB in-router"
    );
    let s147 = include_str!(
        "../docs/EPICS/ZERO_TRUST/epic-14-spiffe-mtls-federation/story-14.7-jwt-svid-hardening-revocation-ecdsa.md"
    );
    assert!(
        s147.contains("No** Redis")
            || s147.contains("No** in-tree IdP")
            || s147.contains("external"),
        "14.7 must forbid in-router IdP revocation product"
    );
}
