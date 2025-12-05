//! # OpenAPI Linter Module
//!
//! Provides comprehensive linting for OpenAPI specifications to ensure they
//! conform to BRRTRouter conventions and best practices.
//!
//! ## Checks Performed
//!
//! 1. **operationId casing** - Must be snake_case (not camelCase)
//! 2. **Schema format consistency** - `required` should be array, properties inline format
//! 3. **Missing type definitions** - All `$ref` references must resolve
//! 4. **Schema completeness** - Schemas should have full typing (type, format, enum)
//! 5. **Missing operationId** - All operations must have operationId
//! 6. **Schema reference resolution** - All $ref paths must be valid
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::linter::{lint_spec, LintIssue, LintSeverity};
//!
//! let issues = lint_spec("path/to/openapi.yaml")?;
//! for issue in &issues {
//!     eprintln!("[{}] {}: {}", issue.severity, issue.location, issue.message);
//! }
//! ```

use oas3::{spec::PathItem, OpenApiV3Spec};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

#[cfg(test)]
mod tests;

/// Severity level for lint issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    /// Error - Will cause code generation to fail
    Error,
    /// Warning - May cause issues but won't block generation
    Warning,
    /// Info - Best practice suggestion
    Info,
}

/// A lint issue found in an OpenAPI specification
#[derive(Debug, Clone)]
pub struct LintIssue {
    /// Where the issue occurred (e.g., "path:/users/{id}/get", "schema:User")
    pub location: String,
    /// Severity of the issue
    pub severity: LintSeverity,
    /// Type of lint issue (e.g., "operation_id_casing", "missing_type")
    pub kind: String,
    /// Human-readable description of the problem
    pub message: String,
    /// Optional suggestion for how to fix it
    pub suggestion: Option<String>,
}

impl LintIssue {
    /// Create a new lint issue
    pub fn new(
        location: impl Into<String>,
        severity: LintSeverity,
        kind: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        LintIssue {
            location: location.into(),
            severity,
            kind: kind.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Add a suggestion for fixing the issue
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Lint an OpenAPI specification file
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI specification file (YAML or JSON)
///
/// # Returns
///
/// A vector of lint issues found in the specification
pub fn lint_spec(spec_path: &Path) -> anyhow::Result<Vec<LintIssue>> {
    let spec: OpenApiV3Spec = if spec_path
        .extension()
        .map(|s| s == "yaml" || s == "yml")
        .unwrap_or(false)
    {
        serde_yaml::from_str(&std::fs::read_to_string(spec_path)?)?
    } else {
        serde_json::from_str(&std::fs::read_to_string(spec_path)?)?
    };

    let mut issues = Vec::new();

    // Collect all defined schema names
    let mut defined_schemas = HashSet::new();
    if let Some(components) = spec.components.as_ref() {
        for (name, _) in &components.schemas {
            defined_schemas.insert(name.clone());
        }
    }

    // Lint all paths and operations
    if let Some(paths) = spec.paths.as_ref() {
        for (path, path_item) in paths {
            lint_path_item(&spec, &mut issues, path, path_item, &defined_schemas);
        }
    }

    // Lint all schemas
    if let Some(components) = spec.components.as_ref() {
        for (name, schema) in &components.schemas {
            lint_schema(&spec, &mut issues, name, schema, &defined_schemas);
        }
    }

    Ok(issues)
}

/// HTTP method enum for linting operations
#[derive(Debug, Clone, Copy)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Trace,
}

impl HttpMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Trace => "TRACE",
        }
    }
}

/// Lint all operations in a PathItem
fn lint_path_item(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    path: &str,
    path_item: &PathItem,
    defined_schemas: &HashSet<String>,
) {
    // Extract all operations from the PathItem using a match-like approach
    let operations: Vec<(HttpMethod, &oas3::spec::Operation)> = vec![
        path_item.get.as_ref().map(|op| (HttpMethod::Get, op)),
        path_item.post.as_ref().map(|op| (HttpMethod::Post, op)),
        path_item.put.as_ref().map(|op| (HttpMethod::Put, op)),
        path_item.delete.as_ref().map(|op| (HttpMethod::Delete, op)),
        path_item.patch.as_ref().map(|op| (HttpMethod::Patch, op)),
        path_item.head.as_ref().map(|op| (HttpMethod::Head, op)),
        path_item
            .options
            .as_ref()
            .map(|op| (HttpMethod::Options, op)),
        path_item.trace.as_ref().map(|op| (HttpMethod::Trace, op)),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Lint each operation
    for (method, operation) in operations {
        lint_operation(
            spec,
            issues,
            path,
            method.as_str(),
            operation,
            defined_schemas,
        );
    }
}

/// Lint a single operation
fn lint_operation(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    path: &str,
    method: &str,
    operation: &oas3::spec::Operation,
    defined_schemas: &HashSet<String>,
) {
    let path_context = format!("{} {}", path, method);
    let operation_id = operation
        .operation_id
        .as_deref()
        .unwrap_or("<no-operationId>");
    let location = format!("{} {}", path_context, operation_id);

    // Check for operationId - use let-else for safe unwrap
    let Some(operation_id) = operation.operation_id.as_ref() else {
        issues.push(
            LintIssue::new(
                &location,
                LintSeverity::Error,
                "missing_operation_id",
                "Operation is missing operationId",
            )
            .with_suggestion("Add operationId field (should be snake_case, e.g., 'get_user')"),
        );
        return; // Can't check casing if no operationId
    };

    // Check operationId casing (must be snake_case)
    if !is_snake_case(operation_id) {
        let suggested = to_snake_case(operation_id);
        issues.push(
            LintIssue::new(
                &location,
                LintSeverity::Error,
                "operation_id_casing",
                format!("operationId '{}' should be snake_case", operation_id),
            )
            .with_suggestion(format!("Change to: {}", suggested)),
        );
    }

    // Check request body schema
    if let Some(request_body) = &operation.request_body {
        lint_request_body(
            spec,
            issues,
            &path_context,
            &location,
            request_body,
            defined_schemas,
        );
    }

    // Check response schemas
    if let Some(responses) = &operation.responses {
        for (status_code, response) in responses {
            lint_response(
                spec,
                issues,
                &path_context,
                &location,
                status_code,
                response,
                defined_schemas,
            );
        }
    }
}

/// Lint a request body
fn lint_request_body(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    path_context: &str,
    _operation_location: &str,
    request_body: &oas3::spec::ObjectOrReference<oas3::spec::RequestBody>,
    defined_schemas: &HashSet<String>,
) {
    let body = match request_body {
        oas3::spec::ObjectOrReference::Object(b) => b,
        oas3::spec::ObjectOrReference::Ref { ref_path } => {
            if !ref_path.starts_with("#/components/requestBodies/") {
                issues.push(LintIssue::new(
                    format!("{} (requestBody)", path_context),
                    LintSeverity::Error,
                    "invalid_request_body_ref",
                    format!("Invalid requestBody $ref: {}", ref_path),
                ));
            }
            return;
        }
    };

    for (_content_type, media_type) in &body.content {
        if let Some(schema_ref) = &media_type.schema {
            lint_schema_ref(
                spec,
                issues,
                &format!("{} (requestBody)", path_context),
                schema_ref,
                defined_schemas,
            );
        }
    }
}

/// Lint a response
fn lint_response(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    path_context: &str,
    _operation_location: &str,
    status_code: &str,
    response: &oas3::spec::ObjectOrReference<oas3::spec::Response>,
    defined_schemas: &HashSet<String>,
) {
    let resp = match response {
        oas3::spec::ObjectOrReference::Object(r) => r,
        oas3::spec::ObjectOrReference::Ref { ref_path } => {
            if !ref_path.starts_with("#/components/responses/") {
                issues.push(LintIssue::new(
                    format!("{} (response {})", path_context, status_code),
                    LintSeverity::Error,
                    "invalid_response_ref",
                    format!("Invalid response $ref: {}", ref_path),
                ));
            }
            return;
        }
    };

    for (_content_type, media_type) in &resp.content {
        if let Some(schema_ref) = &media_type.schema {
            lint_schema_ref(
                spec,
                issues,
                &format!("{} (response {})", path_context, status_code),
                schema_ref,
                defined_schemas,
            );
        }
    }
}

/// Lint a schema reference
fn lint_schema_ref(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    location: &str,
    schema_ref: &oas3::spec::ObjectOrReference<oas3::spec::ObjectSchema>,
    defined_schemas: &HashSet<String>,
) {
    match schema_ref {
        oas3::spec::ObjectOrReference::Object(schema) => {
            lint_schema_object(spec, issues, location, schema, defined_schemas);
        }
        oas3::spec::ObjectOrReference::Ref { ref_path } => {
            // Check if $ref resolves
            if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                if !defined_schemas.contains(name) {
                    issues.push(
                        LintIssue::new(
                            location,
                            LintSeverity::Error,
                            "missing_schema_ref",
                            format!(
                                "Schema reference '{}' not found in components.schemas",
                                name
                            ),
                        )
                        .with_suggestion(format!("Add '{}' to components.schemas", name)),
                    );
                }
            } else {
                issues.push(LintIssue::new(
                    location,
                    LintSeverity::Error,
                    "invalid_schema_ref",
                    format!("Invalid schema $ref path: {}", ref_path),
                ));
            }
        }
    }
}

/// Lint a schema object
fn lint_schema(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    schema_name: &str,
    schema_ref: &oas3::spec::ObjectOrReference<oas3::spec::ObjectSchema>,
    defined_schemas: &HashSet<String>,
) {
    // For schemas defined in components, we don't have path context
    // but we can still show it's a schema definition issue
    let location = format!("schema:{} (components.schemas)", schema_name);
    match schema_ref {
        oas3::spec::ObjectOrReference::Object(schema) => {
            lint_schema_object(spec, issues, &location, schema, defined_schemas);
        }
        oas3::spec::ObjectOrReference::Ref { ref_path: _ } => {
            lint_schema_ref(spec, issues, &location, schema_ref, defined_schemas);
        }
    }
}

/// Lint a schema object (not a reference)
fn lint_schema_object(
    spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    location: &str,
    schema: &oas3::spec::ObjectSchema,
    defined_schemas: &HashSet<String>,
) {
    // Convert schema to JSON for easier inspection
    let schema_json = match serde_json::to_value(schema) {
        Ok(v) => v,
        Err(_) => return, // Skip if we can't serialize
    };

    // Check required field format (should be array, not boolean)
    if let Some(required) = schema_json.get("required") {
        if required.is_boolean() {
            issues.push(
                LintIssue::new(
                    location,
                    LintSeverity::Warning,
                    "required_field_format",
                    "Schema 'required' field should be an array, not a boolean",
                )
                .with_suggestion("Use array format: required: [field1, field2]"),
            );
        }
    }

    // Check properties format (should use inline format like petstore)
    if let Some(properties) = schema_json.get("properties") {
        if let Some(props_obj) = properties.as_object() {
            for (prop_name, prop_value) in props_obj {
                lint_property(
                    spec,
                    issues,
                    &format!("{}.{}", location, prop_name),
                    prop_value,
                    defined_schemas,
                );
            }
        }
    }

    // Check items for array types
    if let Some(items) = schema_json.get("items") {
        if let Some(ref_path) = items.get("$ref").and_then(|v| v.as_str()) {
            if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                if !defined_schemas.contains(name) {
                    issues.push(LintIssue::new(
                        &format!("{}.items", location),
                        LintSeverity::Error,
                        "missing_schema_ref",
                        format!("items $ref '{}' not found", name),
                    ));
                }
            }
        }
    }

    // Check allOf, oneOf, anyOf
    if let Some(all_of) = schema_json.get("allOf") {
        if let Some(all_of_arr) = all_of.as_array() {
            for (idx, item) in all_of_arr.iter().enumerate() {
                if let Some(ref_path) = item.get("$ref").and_then(|v| v.as_str()) {
                    if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                        if !defined_schemas.contains(name) {
                            issues.push(LintIssue::new(
                                &format!("{}.allOf[{}]", location, idx),
                                LintSeverity::Error,
                                "missing_schema_ref",
                                format!("allOf reference '{}' not found", name),
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Lint a property definition
fn lint_property(
    _spec: &OpenApiV3Spec,
    issues: &mut Vec<LintIssue>,
    location: &str,
    property: &Value,
    defined_schemas: &HashSet<String>,
) {
    // Check if property has a valid schema definition
    // Valid schemas can have: type, $ref, const, enum, oneOf, anyOf, allOf
    let has_type = property.get("type").is_some();
    let has_ref = property.get("$ref").is_some();
    let has_const = property.get("const").is_some();
    let has_enum = property.get("enum").is_some();
    let has_one_of = property.get("oneOf").is_some();
    let has_any_of = property.get("anyOf").is_some();
    let has_all_of = property.get("allOf").is_some();

    // Property is valid if it has at least one of these
    if !has_type && !has_ref && !has_const && !has_enum && !has_one_of && !has_any_of && !has_all_of
    {
        issues.push(
            LintIssue::new(
                location,
                LintSeverity::Warning,
                "missing_property_type",
                "Property is missing 'type', '$ref', 'const', 'enum', 'oneOf', 'anyOf', or 'allOf'",
            )
            .with_suggestion(
                "Add type field (e.g., type: string, type: integer) or use const/enum/oneOf",
            ),
        );
    }

    // Check for $ref in property
    if let Some(ref_path) = property.get("$ref").and_then(|v| v.as_str()) {
        if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
            if !defined_schemas.contains(name) {
                issues.push(LintIssue::new(
                    location,
                    LintSeverity::Error,
                    "missing_schema_ref",
                    format!("Property $ref '{}' not found", name),
                ));
            }
        }
    }
}

/// Check if a string is snake_case
pub(crate) fn is_snake_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Must start with lowercase letter or underscore
    if !s
        .chars()
        .next()
        .map(|c| c.is_lowercase() || c == '_')
        .unwrap_or(false)
    {
        return false;
    }
    // Can only contain lowercase letters, digits, and underscores
    s.chars()
        .all(|c| c.is_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Convert a string to snake_case
pub(crate) fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_uppercase() {
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else if ch.is_lowercase() || ch.is_ascii_digit() {
            result.push(ch);
        } else if ch == '-' || ch == ' ' {
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Print lint issues in a formatted way
pub fn print_lint_issues(issues: &[LintIssue]) {
    if issues.is_empty() {
        println!("✅ No lint issues found!");
        return;
    }

    // Group by severity
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Error)
        .collect();
    let warnings: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Warning)
        .collect();
    let infos: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Info)
        .collect();

    println!("\n📋 Lint Results:");
    println!(
        "   {} error(s), {} warning(s), {} info(s)\n",
        errors.len(),
        warnings.len(),
        infos.len()
    );

    if !errors.is_empty() {
        println!("❌ Errors (must fix):");
        for issue in &errors {
            println!("   [{}] {}", issue.kind, issue.location);
            println!("      {}", issue.message);
            if let Some(suggestion) = &issue.suggestion {
                println!("      💡 Suggestion: {}", suggestion);
            }
        }
        println!();
    }

    if !warnings.is_empty() {
        println!("⚠️  Warnings (should fix):");
        for issue in &warnings {
            println!("   [{}] {}", issue.kind, issue.location);
            println!("      {}", issue.message);
            if let Some(suggestion) = &issue.suggestion {
                println!("      💡 Suggestion: {}", suggestion);
            }
        }
        println!();
    }

    if !infos.is_empty() {
        println!("ℹ️  Info (best practices):");
        for issue in &infos {
            println!("   [{}] {}", issue.kind, issue.location);
            println!("      {}", issue.message);
            if let Some(suggestion) = &issue.suggestion {
                println!("      💡 Suggestion: {}", suggestion);
            }
        }
        println!();
    }
}

/// Exit with error code if there are any error-level lint issues
pub fn fail_if_errors(issues: &[LintIssue]) {
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Error)
        .collect();
    if !errors.is_empty() {
        print_lint_issues(issues);
        std::process::exit(1);
    }
}
