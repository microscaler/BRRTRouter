//! # Validator Module
//!
//! The validator module provides OpenAPI specification validation and error reporting
//! for BRRTRouter. It ensures that specifications are well-formed and complete before
//! starting the server.
//!
//! ## Overview
//!
//! This module validates:
//! - Required fields in the OpenAPI specification
//! - Handler name mappings (`operationId` or `x-handler-name`)
//! - Parameter definitions and constraints
//! - Schema references and types
//! - Security scheme definitions
//!
//! Validation happens at startup and can optionally fail fast if issues are found.
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::validator::{ValidationIssue, print_issues, fail_if_issues};
//!
//! let mut issues = Vec::new();
//!
//! // Validate something
//! if some_condition_fails() {
//!     issues.push(ValidationIssue::new(
//!         "/paths/pets/{id}",
//!         "missing_handler",
//!         "Operation is missing operationId or x-handler-name"
//!     ));
//! }
//!
//! // Print issues to stderr
//! if !issues.is_empty() {
//!     print_issues(&issues);
//! }
//!
//! // Or fail fast
//! fail_if_issues(issues);
//!
//! # fn some_condition_fails() -> bool { false }
//! ```

/// Represents a validation issue found in an OpenAPI specification.
///
/// Each issue has:
/// - `location` - Where in the spec the issue was found (e.g., "/paths/pets/{id}")
/// - `kind` - The type of issue (e.g., "missing_handler", "invalid_type")
/// - `message` - A human-readable description of the problem
#[derive(Debug)]
pub struct ValidationIssue {
    pub location: String,
    pub kind: String,
    pub message: String,
}

impl ValidationIssue {
    pub fn new(
        location: impl Into<String>,
        kind: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        ValidationIssue {
            location: location.into(),
            kind: kind.into(),
            message: message.into(),
        }
    }
}

pub fn print_issues(issues: &[ValidationIssue]) {
    eprintln!(
        "\n❌ OpenAPI spec validation failed. {} issue(s) found:\n",
        issues.len()
    );
    for issue in issues {
        eprintln!("[{}] {}: {}", issue.kind, issue.location, issue.message);
    }
    eprintln!("\nPlease fix the issues in your OpenAPI spec before starting the server.\n");
}

pub fn fail_if_issues(issues: Vec<ValidationIssue>) {
    if !issues.is_empty() {
        for issue in &issues {
            eprintln!("[{}] {}: {}", issue.kind, issue.location, issue.message);
        }
        std::process::exit(1);
    }
}
