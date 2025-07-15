#![allow(clippy::result_large_err)]

use super::config::ValidationConfig;
use super::types::{
    ValidationContext, ValidationError, ValidationErrorDetail,
    ValidationErrorType, ValidationResult,
};
use crate::server::ParsedRequest;
use crate::spec::{ParameterLocation, ParameterMeta, RouteMeta};
use jsonschema::JSONSchema;
use serde_json::Value;
use std::collections::HashMap;

/// Request validator with comprehensive parameter validation
pub struct RequestValidator {
    config: ValidationConfig,
}

impl RequestValidator {
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validate complete request against OpenAPI specification
    pub fn validate_request(
        &self,
        route: &RouteMeta,
        request: &ParsedRequest,
        path_params: &HashMap<String, String>,
    ) -> ValidationResult<()> {
        let context = ValidationContext {
            path: route.path_pattern.clone(),
            method: route.method.to_string(),
            strict_mode: self.config.strict_mode,
            development_mode: self.config.development_mode,
        };

        let mut errors = Vec::new();

        // 1. Validate parameters
        if let Err(param_errors) = self.validate_parameters(&route.parameters, request, path_params, &context) {
            errors.extend(param_errors);
        }

        // 2. Validate content type
        if let Err(content_error) = self.validate_content_type(route, request, &context) {
            errors.push(content_error);
        }

        // 3. Validate request size
        if let Err(size_error) = self.validate_request_size(route, request, &context) {
            errors.push(size_error);
        }

        // 4. Enhanced JSON schema validation (if body exists)
        if let Some(body) = &request.body {
            if let Some(schema) = &route.request_schema {
                if let Err(schema_error) = self.validate_json_schema(schema, body, &context) {
                    errors.push(schema_error);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            // Return the first error, but could be enhanced to return all errors
            Err(errors.into_iter().next().unwrap())
        }
    }

    /// Validate all parameters against their OpenAPI definitions
    pub fn validate_parameters(
        &self,
        parameters: &[ParameterMeta],
        request: &ParsedRequest,
        path_params: &HashMap<String, String>,
        _context: &ValidationContext,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for param in parameters {
            if let Err(error) = self.validate_single_parameter(param, request, path_params) {
                errors.push(error);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate a single parameter
    fn validate_single_parameter(
        &self,
        param: &ParameterMeta,
        request: &ParsedRequest,
        path_params: &HashMap<String, String>,
    ) -> ValidationResult<()> {
        let param_config = &self.config.parameter_validation;

        // Get parameter value based on location
        let value = self.get_parameter_value(param, request, path_params);

        // Check required parameters
        if param_config.validate_required && param.required && value.is_none() {
            return Err(ValidationError::new(
                ValidationErrorType::MissingRequiredParameter,
                format!("Required parameter '{}' is missing", param.name),
            )
            .with_field(&param.name)
            .with_location(param.location.to_string()));
        }

        // If parameter is not present and not required, it's valid
        if value.is_none() {
            return Ok(());
        }

        let value_str = value.unwrap();

        // Validate parameter type and constraints if schema is provided
        if let Some(schema) = &param.schema {
            if param_config.validate_types || param_config.validate_constraints {
                let decoded_value = crate::server::decode_param_value(
                    &value_str,
                    Some(schema),
                    param.style,
                    param.explode,
                );

                self.validate_parameter_schema(
                    param,
                    &decoded_value,
                    schema,
                    &value_str,
                )?;
            }
        }

        Ok(())
    }

    /// Get parameter value from request based on location
    fn get_parameter_value(
        &self,
        param: &ParameterMeta,
        request: &ParsedRequest,
        path_params: &HashMap<String, String>,
    ) -> Option<String> {
        match param.location {
            ParameterLocation::Path => path_params.get(&param.name).cloned(),
            ParameterLocation::Query => request.query_params.get(&param.name).cloned(),
            ParameterLocation::Header => {
                let header_name = if self.config.parameter_validation.case_sensitive_headers {
                    &param.name
                } else {
                    &param.name.to_lowercase()
                };
                request.headers.get(header_name).cloned()
            }
            ParameterLocation::Cookie => request.cookies.get(&param.name).cloned(),
        }
    }

    /// Validate parameter against its JSON schema
    fn validate_parameter_schema(
        &self,
        param: &ParameterMeta,
        decoded_value: &Value,
        schema: &Value,
        original_value: &str,
    ) -> ValidationResult<()> {
        let compiled_schema = JSONSchema::compile(schema).map_err(|e| {
            ValidationError::new(
                ValidationErrorType::InvalidSchema,
                format!("Failed to compile schema: {e}"),
            )
        })?;

        let validation_result = compiled_schema.validate(decoded_value);
        match validation_result {
            Ok(()) => Ok(()),
            Err(validation_errors) => {
                let mut details = Vec::new();
                for error in validation_errors {
                    details.push(
                        ValidationErrorDetail::new(
                            &param.name,
                            param.location.to_string(),
                            error.to_string(),
                        )
                        .with_value(original_value),
                    );
                }

                let error_type = if self.is_type_error(decoded_value, schema) {
                    ValidationErrorType::InvalidParameterType
                } else {
                    ValidationErrorType::ConstraintViolation
                };

                Err(ValidationError::new(
                    error_type,
                    format!("Parameter '{}' validation failed", param.name),
                )
                .with_field(&param.name)
                .with_location(param.location.to_string())
                .with_value(original_value)
                .add_detail(details.into_iter().next().unwrap_or_else(|| {
                    ValidationErrorDetail::new(
                        &param.name,
                        param.location.to_string(),
                        "Validation failed",
                    )
                })))
            }
        }
    }

    /// Validate request Content-Type
    pub fn validate_content_type(
        &self,
        route: &RouteMeta,
        request: &ParsedRequest,
        _context: &ValidationContext,
    ) -> ValidationResult<()> {
        // If no request body schema is defined, Content-Type validation is not required
        if route.request_schema.is_none() {
            return Ok(());
        }

        // If body is present, Content-Type should be specified
        if request.body.is_some() {
            let content_type = request.headers.get("content-type");
            
            if content_type.is_none() {
                return Err(ValidationError::new(
                    ValidationErrorType::UnsupportedContentType,
                    "Content-Type header is required for requests with body",
                ));
            }

            let content_type = content_type.unwrap();
            
            // For now, we primarily support application/json
            // This can be extended to support multiple content types from OpenAPI spec
            if !content_type.starts_with("application/json") {
                return Err(ValidationError::new(
                    ValidationErrorType::UnsupportedContentType,
                    format!("Unsupported Content-Type: {content_type}"),
                )
                .with_value(content_type));
            }
        }

        Ok(())
    }

    /// Validate request size
    pub fn validate_request_size(
        &self,
        _route: &RouteMeta,
        request: &ParsedRequest,
        _context: &ValidationContext,
    ) -> ValidationResult<()> {
        if let Some(body) = &request.body {
            let body_size = body.to_string().len();
            if body_size > self.config.max_request_size {
                return Err(ValidationError::new(
                    ValidationErrorType::PayloadTooLarge,
                    format!(
                        "Request body size ({} bytes) exceeds maximum allowed size ({} bytes)",
                        body_size, self.config.max_request_size
                    ),
                )
                .with_value(body_size.to_string()));
            }
        }

        Ok(())
    }

    /// Enhanced JSON schema validation
    fn validate_json_schema(
        &self,
        schema: &Value,
        body: &Value,
        _context: &ValidationContext,
    ) -> ValidationResult<()> {
        let compiled_schema = JSONSchema::compile(schema).map_err(|e| {
            ValidationError::new(
                ValidationErrorType::InvalidSchema,
                format!("Failed to compile schema: {e}"),
            )
        })?;

        let validation_result = compiled_schema.validate(body);
        match validation_result {
            Ok(()) => Ok(()),
            Err(validation_errors) => {
                let mut details = Vec::new();
                for error in validation_errors {
                    details.push(ValidationErrorDetail::new(
                        error.instance_path.to_string(),
                        "body",
                        error.to_string(),
                    ));
                }

                Err(ValidationError::new(
                    ValidationErrorType::InvalidSchema,
                    "Request body validation failed",
                )
                .add_detail(details.into_iter().next().unwrap_or_else(|| {
                    ValidationErrorDetail::new("body", "body", "Schema validation failed")
                })))
            }
        }
    }

    /// Check if validation error is a type error
    fn is_type_error(&self, value: &Value, schema: &Value) -> bool {
        if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
            let actual_type = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            
            // Special case: if expected type is integer and we have a number, check if it's actually an integer
            if expected_type == "integer" && actual_type == "number" {
                if let Some(num) = value.as_f64() {
                    return num.fract() != 0.0; // It's a type error if it has fractional part
                }
            }
            
            expected_type != actual_type
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ParameterLocation, ParameterMeta, ParameterStyle};
    use http::Method;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_request() -> ParsedRequest {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        let mut query_params = HashMap::new();
        query_params.insert("limit".to_string(), "10".to_string());

        ParsedRequest {
            method: "GET".to_string(),
            path: "/test/123".to_string(),
            headers,
            cookies: HashMap::new(),
            query_params,
            body: Some(json!({"name": "test"})),
        }
    }

    fn create_test_path_params() -> HashMap<String, String> {
        let mut path_params = HashMap::new();
        path_params.insert("id".to_string(), "123".to_string());
        path_params
    }

    fn create_test_route() -> RouteMeta {
        RouteMeta {
            method: Method::GET,
            path_pattern: "/test/{id}".to_string(),
            handler_name: "test_handler".to_string(),
            parameters: vec![
                ParameterMeta {
                    name: "id".to_string(),
                    location: ParameterLocation::Path,
                    required: true,
                    schema: Some(json!({"type": "string"})),
                    style: Some(ParameterStyle::Simple),
                    explode: None,
                },
                ParameterMeta {
                    name: "limit".to_string(),
                    location: ParameterLocation::Query,
                    required: false,
                    schema: Some(json!({"type": "integer", "minimum": 1})),
                    style: Some(ParameterStyle::Form),
                    explode: None,
                },
            ],
            request_schema: Some(json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                },
                "required": ["name"]
            })),
            response_schema: None,
            example: None,
            responses: HashMap::new(),
            security: Vec::new(),
            example_name: "test".to_string(),
            project_slug: "test".to_string(),
            output_dir: std::path::PathBuf::new(),
            base_path: String::new(),
            sse: false,
        }
    }

    #[test]
    fn test_valid_request_validation() {
        let validator = RequestValidator::new(ValidationConfig::default());
        let request = create_test_request();
        let path_params = create_test_path_params();
        let route = create_test_route();

        let result = validator.validate_request(&route, &request, &path_params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_required_parameter() {
        let validator = RequestValidator::new(ValidationConfig::default());
        let request = create_test_request();
        let path_params = HashMap::new(); // Remove required 'id' parameter
        let route = create_test_route();

        let result = validator.validate_request(&route, &request, &path_params);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error.error_type, ValidationErrorType::MissingRequiredParameter));
        assert_eq!(error.field, Some("id".to_string()));
    }

    #[test]
    fn test_invalid_parameter_type() {
        let validator = RequestValidator::new(ValidationConfig::default());
        let mut request = create_test_request();
        request.query_params.insert("limit".to_string(), "invalid".to_string());
        let path_params = create_test_path_params();
        let route = create_test_route();

        let result = validator.validate_request(&route, &request, &path_params);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error.error_type, ValidationErrorType::InvalidParameterType));
    }

    #[test]
    fn test_constraint_violation() {
        let validator = RequestValidator::new(ValidationConfig::default());
        let mut request = create_test_request();
        request.query_params.insert("limit".to_string(), "0".to_string()); // Below minimum
        let path_params = create_test_path_params();
        let route = create_test_route();

        let result = validator.validate_request(&route, &request, &path_params);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error.error_type, ValidationErrorType::ConstraintViolation));
    }

    #[test]
    fn test_unsupported_content_type() {
        let validator = RequestValidator::new(ValidationConfig::default());
        let mut request = create_test_request();
        request.headers.insert("content-type".to_string(), "text/plain".to_string());
        let path_params = create_test_path_params();
        let route = create_test_route();

        let result = validator.validate_request(&route, &request, &path_params);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error.error_type, ValidationErrorType::UnsupportedContentType));
    }

    #[test]
    fn test_payload_too_large() {
        let config = ValidationConfig {
            max_request_size: 10, // Very small limit
            ..Default::default()
        };

        let validator = RequestValidator::new(config);
        let request = create_test_request();
        let path_params = create_test_path_params();
        let route = create_test_route();

        let result = validator.validate_request(&route, &request, &path_params);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error.error_type, ValidationErrorType::PayloadTooLarge));
    }
} 