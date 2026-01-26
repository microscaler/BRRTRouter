//! Tests for the Python validator module
//!
//! Note: These tests validate the core logic. Python integration tests
//! should be run separately with Python available.

#[cfg(test)]
mod tests {
    use crate::extract_location_from_error;
    use crate::{ValidationError, ValidationResult};
    use std::path::PathBuf;

    fn test_specs_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("examples")
            .join("pet_store")
            .join("doc")
    }

    #[test]
    fn test_validation_result_new() {
        let result = ValidationResult::new(true, vec![]);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validation_result_with_errors() {
        let errors = vec![ValidationError::new(
            "/paths/test".to_string(),
            "Test error".to_string(),
            "test_error".to_string(),
        )];
        let result = ValidationResult::new(false, errors);
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_validation_error_new() {
        let error = ValidationError::new(
            "/paths/test".to_string(),
            "Test error".to_string(),
            "test_error".to_string(),
        );
        assert_eq!(error.location, "/paths/test");
        assert_eq!(error.message, "Test error");
        assert_eq!(error.kind, "test_error");
    }

    #[test]
    fn test_extract_location_from_error_path() {
        let error_msg = "Error at path: /paths/test\nSome other message";
        let location = extract_location_from_error(error_msg);
        assert_eq!(location, "/paths/test");
    }

    #[test]
    fn test_extract_location_from_error_line_number() {
        let error_msg = "Error at line 42: invalid syntax";
        let location = extract_location_from_error(error_msg);
        assert_eq!(location, "line 42");
    }

    #[test]
    fn test_extract_location_from_error_unknown() {
        let error_msg = "Some generic error message";
        let location = extract_location_from_error(error_msg);
        assert_eq!(location, "unknown");
    }

    #[test]
    fn test_extract_location_from_error_no_match() {
        let error_msg = "Just a plain error message without location info";
        let location = extract_location_from_error(error_msg);
        assert_eq!(location, "unknown");
    }

    // Note: Tests for validate_openapi_spec and validate_openapi_content
    // require Python to be available at link time. These should be tested
    // via Python integration tests or when Python is available.
}
