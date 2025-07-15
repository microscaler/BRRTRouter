#![allow(clippy::result_large_err)]

use super::config::ValidationConfig;
use super::types::{ValidationError, ValidationErrorType, ValidationResult};
use crate::dispatcher::HandlerResponse;
use crate::spec::RouteMeta;
use jsonschema::JSONSchema;
use std::collections::HashMap;

/// Response validator for validating handler responses against OpenAPI specifications
pub struct ResponseValidator {
    config: ValidationConfig,
}

impl ResponseValidator {
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validate response against OpenAPI specification
    pub fn validate_response(
        &self,
        route: &RouteMeta,
        response: &HandlerResponse,
    ) -> ValidationResult<()> {
        // Validate response body against schema if present
        if let Some(schema) = &route.response_schema {
            if self.config.validate_responses {
                return self.validate_response_body_schema(schema, &response.body);
            }
        }

        // TODO: Implement response header validation
        // TODO: Implement response status code validation

        Ok(())
    }

    /// Validate response body against JSON schema
    fn validate_response_body_schema(
        &self,
        schema: &serde_json::Value,
        body: &serde_json::Value,
    ) -> ValidationResult<()> {
        let compiled_schema = JSONSchema::compile(schema).map_err(|e| {
            ValidationError::new(
                ValidationErrorType::InvalidSchema,
                format!("Failed to compile response schema: {e}"),
            )
        })?;

        let validation_result = compiled_schema.validate(body);
        match validation_result {
            Ok(()) => Ok(()),
            Err(validation_errors) => {
                let details: Vec<String> = validation_errors.map(|e| e.to_string()).collect();
                Err(ValidationError::new(
                    ValidationErrorType::InvalidSchema,
                    format!("Response validation failed: {}", details.join(", ")),
                ))
            }
        }
    }

    /// Validate response headers against OpenAPI definitions
    pub fn validate_response_headers(
        &self,
        _route: &RouteMeta,
        _headers: &HashMap<String, String>,
    ) -> ValidationResult<()> {
        // TODO: Implement response header validation
        Ok(())
    }

    /// Validate response status code against OpenAPI definitions
    pub fn validate_response_status(
        &self,
        _route: &RouteMeta,
        _status: u16,
    ) -> ValidationResult<()> {
        // TODO: Implement response status code validation
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::RouteMeta;
    use http::Method;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_response_validator_creation() {
        let config = ValidationConfig::default();
        let validator = ResponseValidator::new(config);
        
        // Basic test to ensure validator can be created
        let route = RouteMeta {
            method: Method::GET,
            path_pattern: "/test".to_string(),
            handler_name: "test".to_string(),
            parameters: Vec::new(),
            request_schema: None,
            response_schema: None,
            example: None,
            responses: HashMap::new(),
            security: Vec::new(),
            example_name: "test".to_string(),
            project_slug: "test".to_string(),
            output_dir: PathBuf::new(),
            base_path: String::new(),
            sse: false,
        };

        let response = HandlerResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({"ok": true}),
        };

        let result = validator.validate_response(&route, &response);
        assert!(result.is_ok());
    }

    #[test]
    fn test_response_validation_with_schema() {
        let config = ValidationConfig::default();
        let validator = ResponseValidator::new(config);
        
        let route = RouteMeta {
            method: Method::GET,
            path_pattern: "/test".to_string(),
            handler_name: "test".to_string(),
            parameters: Vec::new(),
            request_schema: None,
            response_schema: Some(json!({
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            })),
            example: None,
            responses: HashMap::new(),
            security: Vec::new(),
            example_name: "test".to_string(),
            project_slug: "test".to_string(),
            output_dir: PathBuf::new(),
            base_path: String::new(),
            sse: false,
        };

        // Valid response
        let valid_response = HandlerResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({"name": "test"}),
        };

        let result = validator.validate_response(&route, &valid_response);
        assert!(result.is_ok());

        // Invalid response - name should be string, not number
        let invalid_response = HandlerResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({"name": 123}),
        };

        let result = validator.validate_response(&route, &invalid_response);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error.error_type, ValidationErrorType::InvalidSchema));
        assert!(error.message.contains("Response validation failed"));
    }
} 