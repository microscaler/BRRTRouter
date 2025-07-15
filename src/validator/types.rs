use serde::{Deserialize, Serialize};

/// Result type for validation operations
pub type ValidationResult<T = ()> = Result<T, ValidationError>;

/// Comprehensive validation error with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub error_type: ValidationErrorType,
    pub message: String,
    pub field: Option<String>,
    pub location: Option<String>,
    pub constraint: Option<String>,
    pub value: Option<String>,
    pub details: Vec<ValidationErrorDetail>,
}

/// Types of validation errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    MissingRequiredParameter,
    InvalidParameterType,
    ConstraintViolation,
    UnsupportedContentType,
    PayloadTooLarge,
    InvalidSchema,
    InvalidFormat,
}

/// Detailed validation error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorDetail {
    pub field: String,
    pub location: String,
    pub message: String,
    pub constraint: Option<String>,
    pub value: Option<String>,
}

/// Parameter validation configuration
#[derive(Debug, Clone)]
pub struct ParameterValidationConfig {
    pub validate_required: bool,
    pub validate_types: bool,
    pub validate_constraints: bool,
    pub validate_formats: bool,
    pub case_sensitive_headers: bool,
}

impl Default for ParameterValidationConfig {
    fn default() -> Self {
        Self {
            validate_required: true,
            validate_types: true,
            validate_constraints: true,
            validate_formats: true,
            case_sensitive_headers: false,
        }
    }
}

/// Validation context for tracking validation state
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub path: String,
    pub method: String,
    pub strict_mode: bool,
    pub development_mode: bool,
}

/// Header metadata for response validation
#[derive(Debug, Clone)]
pub struct HeaderMeta {
    pub name: String,
    pub required: bool,
    pub schema: Option<serde_json::Value>,
}

impl ValidationError {
    pub fn new(error_type: ValidationErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            message: message.into(),
            field: None,
            location: None,
            constraint: None,
            value: None,
            details: Vec::new(),
        }
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraint = Some(constraint.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn add_detail(mut self, detail: ValidationErrorDetail) -> Self {
        self.details.push(detail);
        self
    }

    /// Convert to HTTP status code
    pub fn to_status_code(&self) -> u16 {
        match self.error_type {
            ValidationErrorType::MissingRequiredParameter => 400,
            ValidationErrorType::InvalidParameterType => 400,
            ValidationErrorType::ConstraintViolation => 400,
            ValidationErrorType::UnsupportedContentType => 415,
            ValidationErrorType::PayloadTooLarge => 413,
            ValidationErrorType::InvalidSchema => 400,
            ValidationErrorType::InvalidFormat => 400,
        }
    }
}

impl ValidationErrorDetail {
    pub fn new(
        field: impl Into<String>,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            location: location.into(),
            message: message.into(),
            constraint: None,
            value: None,
        }
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraint = Some(constraint.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
}
