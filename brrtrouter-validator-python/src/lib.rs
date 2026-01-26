//! # BRRTRouter Python Validator
//!
//! Python bindings for BRRTRouter's OpenAPI specification validation.
//! This module allows Python code to validate OpenAPI YAML/JSON files
//! using the same validation logic that BRRTRouter uses.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::{Deserialize, Serialize};

/// Result of OpenAPI specification validation
#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct ValidationResult {
    /// Whether the specification is valid
    #[pyo3(get)]
    pub valid: bool,
    /// List of validation errors (empty if valid)
    #[pyo3(get)]
    pub errors: Vec<ValidationError>,
}

#[pymethods]
impl ValidationResult {
    #[new]
    fn new(valid: bool, errors: Vec<ValidationError>) -> Self {
        ValidationResult { valid, errors }
    }

    /// Convert to Python dict
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new_bound(py);
        dict.set_item("valid", self.valid)?;
        let errors_list = PyList::empty_bound(py);
        for error in &self.errors {
            let error_dict = PyDict::new_bound(py);
            error_dict.set_item("location", &error.location)?;
            error_dict.set_item("message", &error.message)?;
            error_dict.set_item("kind", &error.kind)?;
            errors_list.append(error_dict)?;
        }
        dict.set_item("errors", errors_list)?;
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        if self.valid {
            "ValidationResult(valid=True, errors=[])".to_string()
        } else {
            format!(
                "ValidationResult(valid=False, errors=[{}])",
                self.errors.len()
            )
        }
    }
}

/// A single validation error
#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct ValidationError {
    /// Location in the spec where the error occurred (e.g., "/paths/pets/{id}")
    #[pyo3(get)]
    pub location: String,
    /// Human-readable error message
    #[pyo3(get)]
    pub message: String,
    /// Type of validation error (e.g., "validation_error", "parse_error")
    #[pyo3(get)]
    pub kind: String,
}

#[pymethods]
impl ValidationError {
    #[new]
    fn new(location: String, message: String, kind: String) -> Self {
        ValidationError {
            location,
            message,
            kind,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationError(location='{}', kind='{}', message='{}')",
            self.location, self.kind, self.message
        )
    }
}

/// Validate an OpenAPI specification file
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI YAML or JSON file
///
/// # Returns
///
/// A `ValidationResult` indicating whether the spec is valid and any errors found.
#[pyfunction]
fn validate_openapi_spec(spec_path: &str) -> PyResult<ValidationResult> {
    match brrtrouter::spec::load_spec(spec_path) {
        Ok(_) => Ok(ValidationResult {
            valid: true,
            errors: vec![],
        }),
        Err(e) => {
            // Parse error to extract meaningful information
            let error_msg = format!("{}", e);
            let location = extract_location_from_error(&error_msg);

            let error = ValidationError {
                location,
                message: error_msg,
                kind: "validation_error".to_string(),
            };

            Ok(ValidationResult {
                valid: false,
                errors: vec![error],
            })
        }
    }
}

/// Validate OpenAPI specification content (YAML or JSON string)
///
/// # Arguments
///
/// * `content` - OpenAPI specification content as a string
/// * `format` - Format of the content: "yaml" or "json"
///
/// # Returns
///
/// A `ValidationResult` indicating whether the spec is valid and any errors found.
#[pyfunction]
fn validate_openapi_content(content: &str, format: &str) -> PyResult<ValidationResult> {
    use brrtrouter::spec::load_spec_from_spec;
    use oas3::OpenApiV3Spec;

    // Parse YAML or JSON content
    let spec: OpenApiV3Spec = match format.to_lowercase().as_str() {
        "yaml" | "yml" => serde_yaml::from_str(content)
            .map_err(|e| PyValueError::new_err(format!("Invalid YAML: {}", e)))?,
        "json" => serde_json::from_str(content)
            .map_err(|e| PyValueError::new_err(format!("Invalid JSON: {}", e)))?,
        _ => {
            return Err(PyValueError::new_err(
                "Format must be 'yaml', 'yml', or 'json'",
            ));
        }
    };

    // Validate using BRRTRouter's validation logic
    match load_spec_from_spec(spec) {
        Ok(_) => Ok(ValidationResult {
            valid: true,
            errors: vec![],
        }),
        Err(e) => {
            let error_msg = format!("{}", e);
            let location = extract_location_from_error(&error_msg);

            let error = ValidationError {
                location,
                message: error_msg,
                kind: "validation_error".to_string(),
            };

            Ok(ValidationResult {
                valid: false,
                errors: vec![error],
            })
        }
    }
}

#[cfg(test)]
mod tests;

/// Extract location information from error message
///
/// Attempts to extract a meaningful location (path, line number, etc.)
/// from the error message for better error reporting.
fn extract_location_from_error(error_msg: &str) -> String {
    // Try to extract path information from common error patterns
    if let Some(path_start) = error_msg.find("path:") {
        if let Some(path_end) = error_msg[path_start..].find('\n') {
            return error_msg[path_start + 5..path_start + path_end]
                .trim()
                .to_string();
        }
    }

    // Try to extract line number
    if let Some(line_start) = error_msg.find("line ") {
        let after_line = &error_msg[line_start + 5..];
        if let Some(line_end) = after_line.find(|c: char| !c.is_ascii_digit()) {
            if line_end > 0 {
                let line_num = &after_line[..line_end];
                if let Ok(_) = line_num.parse::<usize>() {
                    return format!("line {}", line_num);
                }
            }
        }
    }

    // Default to "unknown" if we can't extract location
    "unknown".to_string()
}

/// Python module definition
#[pymodule]
fn brrtrouter_validator(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(validate_openapi_spec, m)?)?;
    m.add_function(wrap_pyfunction!(validate_openapi_content, m)?)?;
    m.add_class::<ValidationResult>()?;
    m.add_class::<ValidationError>()?;
    Ok(())
}
