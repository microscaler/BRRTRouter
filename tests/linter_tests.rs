//! Integration tests for the OpenAPI linter
//!
//! These tests verify that the linter correctly identifies issues in sample
//! OpenAPI specification files.

use brrtrouter::linter::{lint_spec, LintSeverity};
use std::path::PathBuf;

fn test_file_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("linter")
        .join(name)
}

#[test]
fn test_bad_operation_id_casing() {
    let spec_path = test_file_path("bad_operation_id_casing.yaml");
    let issues = lint_spec(&spec_path).unwrap();

    // Should find 2 operationId casing errors
    let casing_errors: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "operation_id_casing" && i.severity == LintSeverity::Error)
        .collect();

    assert_eq!(
        casing_errors.len(),
        2,
        "Should find 2 operationId casing errors. Found: {:?}",
        issues
    );

    // Verify the locations include path context
    let locations: Vec<_> = casing_errors.iter().map(|i| &i.location).collect();
    assert!(
        locations.iter().any(|l| l.contains("/users GET")),
        "Should include /users GET in location"
    );
    assert!(
        locations
            .iter()
            .any(|l| l.contains("/users/{id} POST") || l.contains(r"/users/{id} POST")),
        "Should include /users/{{id}} POST in location"
    );

    // Verify suggestions are provided
    assert!(casing_errors.iter().all(|i| i.suggestion.is_some()));
}

#[test]
fn test_missing_operation_id() {
    let spec_path = test_file_path("missing_operation_id.yaml");
    let issues = lint_spec(&spec_path).unwrap();

    // Should find 2 missing operationId errors
    let missing_op_id_errors: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_operation_id" && i.severity == LintSeverity::Error)
        .collect();

    assert_eq!(
        missing_op_id_errors.len(),
        2,
        "Should find 2 missing operationId errors. Found: {:?}",
        issues
    );

    // Verify locations include path context
    let locations: Vec<_> = missing_op_id_errors.iter().map(|i| &i.location).collect();
    assert!(
        locations.iter().any(|l| l.contains("/pets GET")),
        "Should include /pets GET in location"
    );
    assert!(
        locations
            .iter()
            .any(|l| l.contains("/pets/{id} DELETE") || l.contains(r"/pets/{id} DELETE")),
        "Should include /pets/{{id}} DELETE in location"
    );
}

#[test]
fn test_missing_schema_ref() {
    let spec_path = test_file_path("missing_schema_ref.yaml");
    let issues = lint_spec(&spec_path).unwrap();

    // Should find missing schema reference errors
    let missing_ref_errors: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_schema_ref" && i.severity == LintSeverity::Error)
        .collect();

    assert!(
        missing_ref_errors.len() >= 2,
        "Should find at least 2 missing schema reference errors. Found: {:?}",
        issues
    );

    // Verify one is for UserList in response
    let user_list_error = missing_ref_errors
        .iter()
        .find(|i| i.message.contains("UserList"));
    assert!(
        user_list_error.is_some(),
        "Should find error for missing UserList schema"
    );
    assert!(
        user_list_error.unwrap().location.contains("/users GET"),
        "Should include path context in location"
    );

    // Verify one is for Account in schema property
    let account_error = missing_ref_errors
        .iter()
        .find(|i| i.message.contains("Account"));
    assert!(
        account_error.is_some(),
        "Should find error for missing Account schema"
    );
}

#[test]
fn test_invalid_schema_format() {
    let spec_path = test_file_path("invalid_schema_format.yaml");
    let issues = lint_spec(&spec_path).unwrap();

    // Note: required: true is invalid OpenAPI syntax, so the parser rejects it
    // We test for missing property type warnings instead

    // Should find missing property type warnings
    let missing_type_warnings: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_property_type" && i.severity == LintSeverity::Warning)
        .collect();

    assert!(
        missing_type_warnings.len() >= 2,
        "Should find at least 2 missing property type warnings. Found: {:?}",
        issues
    );
}

#[test]
fn test_mixed_issues() {
    let spec_path = test_file_path("mixed_issues.yaml");
    let issues = lint_spec(&spec_path).unwrap();

    // Should find multiple types of issues
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Error)
        .collect();
    let warnings: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Warning)
        .collect();

    // Should have at least:
    // - 1 operationId casing error (getOrders)
    // - 1 missing operationId error (POST /orders)
    // - Multiple missing schema ref errors (OrderList, CreateOrderRequest, Customer)
    assert!(
        errors.len() >= 4,
        "Should find at least 4 errors. Found: {} errors, {} warnings. Issues: {:?}",
        errors.len(),
        warnings.len(),
        issues
    );

    // Verify operationId casing error
    let casing_error = errors.iter().find(|i| i.kind == "operation_id_casing");
    assert!(
        casing_error.is_some(),
        "Should find operationId casing error"
    );
    assert!(
        casing_error.unwrap().location.contains("/orders GET"),
        "Should include path context"
    );

    // Verify missing operationId error
    let missing_op_id = errors.iter().find(|i| i.kind == "missing_operation_id");
    assert!(
        missing_op_id.is_some(),
        "Should find missing operationId error"
    );
    assert!(
        missing_op_id.unwrap().location.contains("/orders POST"),
        "Should include path context"
    );

    // Verify missing schema ref errors (at least OrderList and Customer)
    let missing_ref_errors: Vec<_> = errors
        .iter()
        .filter(|i| i.kind == "missing_schema_ref")
        .collect();
    assert!(
        missing_ref_errors.len() >= 2,
        "Should find at least 2 missing schema ref errors (OrderList, Customer). Found: {:?}",
        missing_ref_errors
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );

    // Note: enum without type is valid JSON Schema, so no warning expected
    // Warnings are optional - we just verify errors are found
}

#[test]
fn test_all_sample_files_exist() {
    // Verify all test files can be loaded
    let test_files = vec![
        "bad_operation_id_casing.yaml",
        "missing_operation_id.yaml",
        "missing_schema_ref.yaml",
        "invalid_schema_format.yaml",
        "mixed_issues.yaml",
    ];

    for file in test_files {
        let path = test_file_path(file);
        assert!(path.exists(), "Test file should exist: {}", path.display());
        // Verify it can be parsed
        let issues = lint_spec(&path);
        assert!(
            issues.is_ok(),
            "Should be able to lint file: {}",
            path.display()
        );
    }
}
