#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Unit tests for the OpenAPI linter

use crate::linter::{lint_spec, LintSeverity};
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

/// Helper to create a temp file with YAML content and run lint_spec on it
fn lint_yaml(content: &str) -> Vec<crate::linter::LintIssue> {
    let mut temp = NamedTempFile::with_suffix(".yaml").expect("create temp file");
    temp.write_all(content.as_bytes()).expect("write spec");
    temp.flush().expect("flush");
    lint_spec(temp.path()).expect("lint spec")
}

#[test]
fn test_lint_missing_operation_id() {
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /test:
    get:
      summary: Test endpoint
      responses:
        '200':
          description: OK
"#;

    let issues = lint_yaml(spec);
    let missing_op_id_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_operation_id")
        .collect();

    assert!(
        !missing_op_id_issues.is_empty(),
        "Should detect missing operationId"
    );
    assert_eq!(missing_op_id_issues[0].severity, LintSeverity::Error);
}

#[test]
fn test_lint_operation_id_casing() {
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /test:
    get:
      operationId: getTest
      summary: Test endpoint
      responses:
        '200':
          description: OK
"#;

    let issues = lint_yaml(spec);
    let casing_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "operation_id_casing")
        .collect();

    assert!(
        !casing_issues.is_empty(),
        "Should detect camelCase operationId"
    );
    assert_eq!(casing_issues[0].severity, LintSeverity::Error);
    assert!(
        casing_issues[0].suggestion.is_some(),
        "Should provide suggestion"
    );
}

#[test]
fn test_lint_snake_case_operation_id_passes() {
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /test:
    get:
      operationId: get_test
      summary: Test endpoint
      responses:
        '200':
          description: OK
"#;

    let issues = lint_yaml(spec);
    let casing_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "operation_id_casing")
        .collect();

    assert!(
        casing_issues.is_empty(),
        "Should not flag snake_case operationId"
    );
}

#[test]
fn test_lint_missing_schema_ref() {
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /test:
    get:
      operationId: get_test
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/NonExistent'
components:
  schemas: {}
"#;

    let issues = lint_yaml(spec);
    let missing_ref_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_schema_ref")
        .collect();

    assert!(
        !missing_ref_issues.is_empty(),
        "Should detect missing schema reference"
    );
    assert_eq!(missing_ref_issues[0].severity, LintSeverity::Error);
}

#[test]
fn test_lint_valid_schema_ref() {
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /test:
    get:
      operationId: get_test
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/TestResponse'
components:
  schemas:
    TestResponse:
      type: object
      properties:
        message:
          type: string
"#;

    let issues = lint_yaml(spec);
    let missing_ref_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_schema_ref")
        .collect();

    assert!(
        missing_ref_issues.is_empty(),
        "Should not flag valid schema reference"
    );
}

#[test]
fn test_lint_required_field_format() {
    // Note: OpenAPI 3.x doesn't allow boolean 'required' at schema level,
    // but we test the linter's detection logic by checking the JSON representation
    // This test verifies the linter would catch it if it existed in the parsed schema
    // For now, we'll test that the linter correctly handles array format
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
components:
  schemas:
    TestSchema:
      type: object
      required: [field]
      properties:
        field:
          type: string
"#;

    let issues = lint_yaml(spec);
    let required_format_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "required_field_format")
        .collect();

    // Array format should not trigger the warning
    assert!(
        required_format_issues.is_empty(),
        "Should not flag array format for required field"
    );
}

#[test]
fn test_lint_missing_property_type() {
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
components:
  schemas:
    TestSchema:
      type: object
      properties:
        bad_field:
          description: Missing type
"#;

    let issues = lint_yaml(spec);
    let missing_type_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "missing_property_type")
        .collect();

    assert!(
        !missing_type_issues.is_empty(),
        "Should detect missing property type"
    );
    assert_eq!(missing_type_issues[0].severity, LintSeverity::Warning);
}

#[test]
fn test_lint_petstore_example() {
    // Test with the actual petstore example to ensure it passes
    let petstore_spec = Path::new("examples/openapi.yaml");
    if petstore_spec.exists() {
        let issues = lint_spec(petstore_spec).unwrap();
        let errors: Vec<_> = issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Error)
            .collect();

        // Petstore should have no errors (it's our reference example)
        assert!(
            errors.is_empty(),
            "Petstore example should have no lint errors. Found: {:?}",
            errors
                .iter()
                .map(|e| format!("{}: {}", e.location, e.message))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn test_is_snake_case() {
    use crate::linter::is_snake_case;

    assert!(is_snake_case("get_user"));
    assert!(is_snake_case("create_account"));
    assert!(is_snake_case("get_user_by_id"));
    assert!(is_snake_case("_internal"));
    assert!(is_snake_case("user123"));

    assert!(!is_snake_case("getUser"));
    assert!(!is_snake_case("GetUser"));
    assert!(!is_snake_case("get-User"));
    assert!(!is_snake_case(""));
}

#[test]
fn test_to_snake_case() {
    use crate::linter::to_snake_case;

    assert_eq!(to_snake_case("getUser"), "get_user");
    assert_eq!(to_snake_case("GetUser"), "get_user");
    assert_eq!(to_snake_case("createAccount"), "create_account");
    assert_eq!(to_snake_case("getUserById"), "get_user_by_id");
    assert_eq!(to_snake_case("get-user"), "get_user");
}

#[test]
fn test_lint_money_type_without_decimal_format() {
    // type: number without format: decimal for money-like property should warn
    let spec = r#"
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /payments:
    get:
      operationId: list_payments
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  payment_amount:
                    type: number
                  total_balance:
                    type: number
                  currency_code:
                    type: string
components:
  schemas:
    Payment:
      type: object
      properties:
        amount:
          type: number
        applied_amount:
          type: number
          format: decimal
"#;

    let issues = lint_yaml(spec);
    let money_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == "money_type_without_decimal_format")
        .collect();

    // payment_amount and total_balance in response schema; amount in Payment (applied_amount has format: decimal)
    assert!(
        money_issues.len() >= 2,
        "Should warn for type: number without format on money-like properties. Found: {:?}",
        issues
    );
    assert!(money_issues
        .iter()
        .all(|i| i.severity == LintSeverity::Warning));
    assert!(money_issues.iter().all(|i| i.suggestion.is_some()));
}
