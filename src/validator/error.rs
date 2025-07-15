use super::types::{ValidationError, ValidationErrorDetail};
use may_minihttp::Response;
use serde_json::{json, Value};
use std::collections::HashMap;

/// RFC 7807 Problem Details response
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub instance: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub validation_errors: Vec<ValidationErrorDetail>,
    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

impl ProblemDetails {
    pub fn new(
        problem_type: impl Into<String>,
        title: impl Into<String>,
        status: u16,
        detail: impl Into<String>,
        instance: impl Into<String>,
    ) -> Self {
        Self {
            problem_type: problem_type.into(),
            title: title.into(),
            status,
            detail: detail.into(),
            instance: instance.into(),
            validation_errors: Vec::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn with_validation_errors(mut self, errors: Vec<ValidationErrorDetail>) -> Self {
        self.validation_errors = errors;
        self
    }

    pub fn add_extension(mut self, key: impl Into<String>, value: Value) -> Self {
        self.extensions.insert(key.into(), value);
        self
    }
}

impl From<ValidationError> for ProblemDetails {
    fn from(error: ValidationError) -> Self {
        let status = error.to_status_code();
        let title = match error.error_type {
            super::types::ValidationErrorType::MissingRequiredParameter => {
                "Missing Required Parameter"
            }
            super::types::ValidationErrorType::InvalidParameterType => "Invalid Parameter Type",
            super::types::ValidationErrorType::ConstraintViolation => {
                "Parameter Constraint Violation"
            }
            super::types::ValidationErrorType::UnsupportedContentType => "Unsupported Media Type",
            super::types::ValidationErrorType::PayloadTooLarge => "Payload Too Large",
            super::types::ValidationErrorType::InvalidSchema => "Invalid Schema",
            super::types::ValidationErrorType::InvalidFormat => "Invalid Format",
        };

        let mut problem = ProblemDetails::new(
            "https://brrtrouter.dev/problems/validation-error",
            title,
            status,
            error.message,
            "/", // Will be updated with actual path
        );

        if !error.details.is_empty() {
            problem = problem.with_validation_errors(error.details);
        }

        // Add field-specific information as extensions
        if let Some(field) = error.field {
            problem = problem.add_extension("field", json!(field));
        }
        if let Some(location) = error.location {
            problem = problem.add_extension("location", json!(location));
        }
        if let Some(constraint) = error.constraint {
            problem = problem.add_extension("constraint", json!(constraint));
        }
        if let Some(value) = error.value {
            problem = problem.add_extension("value", json!(value));
        }

        problem
    }
}

/// Write validation error response to HTTP response
pub fn write_validation_error(
    res: &mut Response,
    error: ValidationError,
    request_path: &str,
) -> std::io::Result<()> {
    let mut problem = ProblemDetails::from(error);
    problem.instance = request_path.to_string();

    let status = problem.status;
    let reason = status_reason(status);

    res.status_code(status as usize, reason);
    res.header("Content-Type: application/problem+json");

    let body = serde_json::to_vec(&problem).unwrap_or_else(|_| {
        json!({
            "type": "https://brrtrouter.dev/problems/internal-error",
            "title": "Internal Server Error",
            "status": 500,
            "detail": "Failed to serialize error response"
        })
        .to_string()
        .into_bytes()
    });

    res.body_vec(body);
    Ok(())
}

/// Write multiple validation errors as a single response
pub fn write_validation_errors(
    res: &mut Response,
    errors: Vec<ValidationError>,
    request_path: &str,
) -> std::io::Result<()> {
    if errors.is_empty() {
        return Ok(());
    }

    // Use the first error as the primary error, combine details
    let mut primary_error = errors[0].clone();

    // Collect all validation details
    let mut all_details = primary_error.details.clone();
    for error in errors.iter().skip(1) {
        all_details.extend(error.details.clone());
    }

    primary_error.details = all_details;

    // Update message to indicate multiple errors
    if errors.len() > 1 {
        primary_error.message = format!(
            "Multiple validation errors occurred ({} errors)",
            errors.len()
        );
    }

    write_validation_error(res, primary_error, request_path)
}

fn status_reason(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::types::ValidationErrorType;

    #[test]
    fn test_problem_details_creation() {
        let problem = ProblemDetails::new(
            "https://example.com/problems/validation",
            "Validation Error",
            400,
            "Parameter validation failed",
            "/test/path",
        );

        assert_eq!(
            problem.problem_type,
            "https://example.com/problems/validation"
        );
        assert_eq!(problem.title, "Validation Error");
        assert_eq!(problem.status, 400);
        assert_eq!(problem.detail, "Parameter validation failed");
        assert_eq!(problem.instance, "/test/path");
    }

    #[test]
    fn test_validation_error_to_problem_details() {
        let error = ValidationError::new(
            ValidationErrorType::MissingRequiredParameter,
            "Parameter 'name' is required",
        )
        .with_field("name")
        .with_location("query");

        let problem = ProblemDetails::from(error);

        assert_eq!(problem.status, 400);
        assert_eq!(problem.title, "Missing Required Parameter");
        assert_eq!(problem.detail, "Parameter 'name' is required");
        assert_eq!(problem.extensions.get("field"), Some(&json!("name")));
        assert_eq!(problem.extensions.get("location"), Some(&json!("query")));
    }

    #[test]
    fn test_validation_error_detail_creation() {
        let detail = ValidationErrorDetail::new("age", "query", "Value must be a positive integer")
            .with_constraint("minimum")
            .with_value("-5");

        assert_eq!(detail.field, "age");
        assert_eq!(detail.location, "query");
        assert_eq!(detail.message, "Value must be a positive integer");
        assert_eq!(detail.constraint, Some("minimum".to_string()));
        assert_eq!(detail.value, Some("-5".to_string()));
    }
}
