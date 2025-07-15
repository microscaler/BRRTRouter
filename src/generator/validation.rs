use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Template validation errors
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Template file not found
    TemplateNotFound(PathBuf),
    /// Template syntax error
    SyntaxError(String),
    /// Missing required template variables
    MissingVariables(Vec<String>),
    /// Template compilation error
    CompilationError(String),
    /// Generated code doesn't compile
    GeneratedCodeError(String),
    /// Template produces invalid output
    InvalidOutput(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::TemplateNotFound(path) => {
                write!(f, "Template not found: {}", path.display())
            }
            ValidationError::SyntaxError(msg) => write!(f, "Template syntax error: {msg}"),
            ValidationError::MissingVariables(vars) => {
                write!(f, "Missing template variables: {}", vars.join(", "))
            }
            ValidationError::CompilationError(msg) => {
                write!(f, "Template compilation error: {msg}")
            }
            ValidationError::GeneratedCodeError(msg) => write!(f, "Generated code error: {msg}"),
            ValidationError::InvalidOutput(msg) => write!(f, "Invalid template output: {msg}"),
        }
    }
}

impl std::error::Error for ValidationError {}

impl From<anyhow::Error> for ValidationError {
    fn from(err: anyhow::Error) -> Self {
        ValidationError::InvalidOutput(err.to_string())
    }
}

impl From<std::io::Error> for ValidationError {
    fn from(err: std::io::Error) -> Self {
        ValidationError::TemplateNotFound(std::path::PathBuf::from(err.to_string()))
    }
}

/// Template validation result
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Template validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Template directory path
    pub template_dir: PathBuf,
    /// Whether to validate generated code compilation
    pub validate_compilation: bool,
    /// Whether to validate template syntax
    pub validate_syntax: bool,
    /// Whether to validate required variables
    pub validate_variables: bool,
    /// Whether to validate output format
    pub validate_output: bool,
    /// Maximum template size in bytes
    pub max_template_size: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            template_dir: PathBuf::from("templates"),
            validate_compilation: true,
            validate_syntax: true,
            validate_variables: true,
            validate_output: true,
            max_template_size: 1024 * 1024, // 1MB
        }
    }
}

/// Template metadata for validation
#[derive(Debug, Clone)]
pub struct TemplateMetadata {
    /// Template file path
    pub path: PathBuf,
    /// Template name
    pub name: String,
    /// Required template variables
    pub required_variables: Vec<String>,
    /// Template type (rust, toml, html, etc.)
    pub template_type: TemplateType,
    /// Template size in bytes
    pub size: usize,
}

/// Template types supported by the system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateType {
    Rust,
    Toml,
    Html,
    Text,
    Unknown,
}

impl TemplateType {
    /// Get template type from file extension
    pub fn from_path(path: &Path) -> Self {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        // Check for template patterns (before .txt extension)
        if filename.contains(".rs.") {
            return Self::Rust;
        }
        if filename.contains(".toml.") || filename.starts_with("Cargo.toml") {
            return Self::Toml;
        }
        if filename.contains(".html.") || filename.contains("index.html") {
            return Self::Html;
        }

        // Check for direct file extensions
        if filename.ends_with(".rs") {
            return Self::Rust;
        }
        if filename.ends_with(".toml") {
            return Self::Toml;
        }
        if filename.ends_with(".html") {
            return Self::Html;
        }

        // Fall back to extension-based detection
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => Self::Rust,
            Some("toml") => Self::Toml,
            Some("html") => Self::Html,
            Some("txt") => Self::Text,
            _ => Self::Unknown,
        }
    }

    /// Get expected file extensions for this template type
    pub fn expected_extensions(&self) -> Vec<&'static str> {
        match self {
            Self::Rust => vec!["rs", "txt"],
            Self::Toml => vec!["toml", "txt"],
            Self::Html => vec!["html", "txt"],
            Self::Text => vec!["txt"],
            Self::Unknown => vec![],
        }
    }
}

/// Main template validator
pub struct TemplateValidator {
    config: ValidationConfig,
    metadata_cache: HashMap<PathBuf, TemplateMetadata>,
}

impl TemplateValidator {
    /// Create a new template validator
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            metadata_cache: HashMap::new(),
        }
    }

    /// Create a validator with default configuration
    pub fn default() -> Self {
        Self::new(ValidationConfig::default())
    }

    /// Validate all templates in the template directory
    pub fn validate_all(&mut self) -> ValidationResult<ValidationReport> {
        let mut report = ValidationReport::new();

        // Discover all template files
        let templates = self.discover_templates()?;

        for template_path in &templates {
            match self.validate_template(template_path) {
                Ok(metadata) => {
                    report.add_success(metadata);
                }
                Err(error) => {
                    report.add_error(template_path.clone(), error);
                }
            }
        }

        Ok(report)
    }

    /// Validate a specific template file
    pub fn validate_template(
        &mut self,
        template_path: &Path,
    ) -> ValidationResult<TemplateMetadata> {
        // Check if template file exists
        if !template_path.exists() {
            return Err(ValidationError::TemplateNotFound(
                template_path.to_path_buf(),
            ));
        }

        // Load template metadata
        let metadata = self.load_template_metadata(template_path)?;

        // Validate template size
        if metadata.size > self.config.max_template_size {
            return Err(ValidationError::InvalidOutput(format!(
                "Template size {} exceeds maximum {} bytes",
                metadata.size, self.config.max_template_size
            )));
        }

        // Validate template syntax
        if self.config.validate_syntax {
            self.validate_template_syntax(&metadata)?;
        }

        // Validate required variables
        if self.config.validate_variables {
            self.validate_template_variables(&metadata)?;
        }

        // Validate output format
        if self.config.validate_output {
            self.validate_template_output(&metadata)?;
        }

        // Cache metadata
        self.metadata_cache
            .insert(template_path.to_path_buf(), metadata.clone());

        Ok(metadata)
    }

    /// Discover all template files in the template directory
    fn discover_templates(&self) -> ValidationResult<Vec<PathBuf>> {
        let mut templates = Vec::new();

        if !self.config.template_dir.exists() {
            return Err(ValidationError::TemplateNotFound(
                self.config.template_dir.clone(),
            ));
        }

        self.walk_directory(&self.config.template_dir, &mut templates)?;
        Ok(templates)
    }

    /// Recursively walk directory to find template files
    fn walk_directory(&self, dir: &Path, templates: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.walk_directory(&path, templates)?;
            } else if self.is_template_file(&path) {
                templates.push(path);
            }
        }
        Ok(())
    }

    /// Check if a file is a template file
    fn is_template_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            matches!(ext, "txt" | "rs" | "toml" | "html")
        } else {
            false
        }
    }

    /// Load template metadata
    fn load_template_metadata(&self, template_path: &Path) -> ValidationResult<TemplateMetadata> {
        let content = fs::read_to_string(template_path)
            .map_err(|_| ValidationError::TemplateNotFound(template_path.to_path_buf()))?;

        let name = template_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        let template_type = TemplateType::from_path(template_path);
        let required_variables = self.extract_template_variables(&content);

        Ok(TemplateMetadata {
            path: template_path.to_path_buf(),
            name,
            required_variables,
            template_type,
            size: content.len(),
        })
    }

    /// Extract template variables from content
    fn extract_template_variables(&self, content: &str) -> Vec<String> {
        let mut variables = HashSet::new();

        // Simple pattern matching for {{ variable }} without regex for now
        let mut chars = content.chars().peekable();
        let mut current_var = String::new();
        let mut in_variable = false;

        while let Some(ch) = chars.next() {
            if ch == '{' && chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'
                in_variable = true;
                current_var.clear();
            } else if ch == '}' && chars.peek() == Some(&'}') && in_variable {
                chars.next(); // consume second '}'
                in_variable = false;
                let var = current_var.trim();
                if !var.is_empty()
                    && var
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_alphabetic() || c == '_')
                {
                    // Insert the full variable name (including dots)
                    variables.insert(var.to_string());
                    // Also insert the main variable name (before any dots) for compatibility
                    let main_var = var.split('.').next().unwrap_or(var);
                    variables.insert(main_var.to_string());
                }
                current_var.clear();
            } else if in_variable {
                current_var.push(ch);
            }
        }

        // Also look for control structures like {% for item in items %}
        let lines: Vec<&str> = content.lines().collect();
        for line in lines {
            if let Some(start) = line.find("{% for ") {
                if let Some(end) = line.find(" %}") {
                    let control_part = &line[start + 7..end];
                    if let Some(in_pos) = control_part.find(" in ") {
                        let var_part = &control_part[in_pos + 4..];
                        let var_name = var_part.trim().split_whitespace().next().unwrap_or("");
                        if !var_name.is_empty() {
                            variables.insert(var_name.to_string());
                        }
                    }
                }
            }
        }

        variables.into_iter().collect()
    }

    /// Validate template syntax
    fn validate_template_syntax(&self, metadata: &TemplateMetadata) -> ValidationResult<()> {
        let content = fs::read_to_string(&metadata.path)
            .map_err(|_| ValidationError::TemplateNotFound(metadata.path.clone()))?;

        // Basic Jinja2 syntax validation
        self.validate_jinja2_syntax(&content)?;

        Ok(())
    }

    /// Validate Jinja2 template syntax
    fn validate_jinja2_syntax(&self, content: &str) -> ValidationResult<()> {
        let mut brace_stack = Vec::new();
        let mut in_variable = false;
        let mut in_control = false;

        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
                if in_variable || in_control {
                    return Err(ValidationError::SyntaxError(
                        "Nested template expressions not allowed".to_string(),
                    ));
                }
                in_variable = true;
                brace_stack.push("{{");
                i += 2;
            } else if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '%' {
                if in_variable || in_control {
                    return Err(ValidationError::SyntaxError(
                        "Nested template expressions not allowed".to_string(),
                    ));
                }
                in_control = true;
                brace_stack.push("{%");
                i += 2;
            } else if i + 1 < chars.len() && chars[i] == '}' && chars[i + 1] == '}' {
                if !in_variable {
                    return Err(ValidationError::SyntaxError(
                        "Unexpected closing variable braces".to_string(),
                    ));
                }
                if let Some(last) = brace_stack.pop() {
                    if last != "{{" {
                        return Err(ValidationError::SyntaxError(
                            "Mismatched template braces".to_string(),
                        ));
                    }
                }
                in_variable = false;
                i += 2;
            } else if i + 1 < chars.len() && chars[i] == '%' && chars[i + 1] == '}' {
                if !in_control {
                    return Err(ValidationError::SyntaxError(
                        "Unexpected closing control braces".to_string(),
                    ));
                }
                if let Some(last) = brace_stack.pop() {
                    if last != "{%" {
                        return Err(ValidationError::SyntaxError(
                            "Mismatched template braces".to_string(),
                        ));
                    }
                }
                in_control = false;
                i += 2;
            } else {
                i += 1;
            }
        }

        if !brace_stack.is_empty() {
            return Err(ValidationError::SyntaxError(
                "Unclosed template expressions".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate template variables
    fn validate_template_variables(&self, metadata: &TemplateMetadata) -> ValidationResult<()> {
        // For now, just validate that required variables are extractable
        // In a more complete implementation, we would check against expected template data structures
        if metadata.required_variables.is_empty() && metadata.template_type != TemplateType::Html {
            return Err(ValidationError::MissingVariables(vec![
                "No template variables found in non-HTML template".to_string(),
            ]));
        }

        Ok(())
    }

    /// Validate template output format
    fn validate_template_output(&self, metadata: &TemplateMetadata) -> ValidationResult<()> {
        let content = fs::read_to_string(&metadata.path)
            .map_err(|_| ValidationError::TemplateNotFound(metadata.path.clone()))?;

        match metadata.template_type {
            TemplateType::Rust => self.validate_rust_template_output(&content)?,
            TemplateType::Toml => self.validate_toml_template_output(&content)?,
            TemplateType::Html => self.validate_html_template_output(&content)?,
            TemplateType::Text => self.validate_text_template_output(&content)?,
            TemplateType::Unknown => {
                return Err(ValidationError::InvalidOutput(
                    "Unknown template type".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate Rust template output structure
    fn validate_rust_template_output(&self, content: &str) -> ValidationResult<()> {
        // Check for common Rust patterns
        let has_use_statements = content.contains("use ");
        let has_struct_or_fn =
            content.contains("struct ") || content.contains("fn ") || content.contains("impl ");

        // For Rust templates, we expect either imports or function/struct definitions
        if !has_use_statements && !has_struct_or_fn {
            return Err(ValidationError::InvalidOutput(
                "Rust template should contain use statements or function/struct definitions"
                    .to_string(),
            ));
        }

        // Check for DO NOT EDIT warning
        if !content.contains("AUTO-GENERATED CODE - DO NOT EDIT") {
            return Err(ValidationError::InvalidOutput(
                "Rust template missing DO NOT EDIT warning".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate TOML template output structure
    fn validate_toml_template_output(&self, content: &str) -> ValidationResult<()> {
        // Basic TOML structure validation
        if !content.contains("[") {
            return Err(ValidationError::InvalidOutput(
                "TOML template should contain section headers".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate HTML template output structure
    fn validate_html_template_output(&self, content: &str) -> ValidationResult<()> {
        // Basic HTML structure validation
        if !content.contains("<!DOCTYPE") && !content.contains("<html") {
            return Err(ValidationError::InvalidOutput(
                "HTML template should contain DOCTYPE or html tag".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate text template output
    fn validate_text_template_output(&self, _content: &str) -> ValidationResult<()> {
        // Text templates are generally flexible
        Ok(())
    }
}

/// Template validation report
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Successfully validated templates
    pub successful: Vec<TemplateMetadata>,
    /// Templates with validation errors
    pub failed: Vec<(PathBuf, ValidationError)>,
    /// Total templates processed
    pub total_processed: usize,
}

impl ValidationReport {
    /// Create a new empty validation report
    pub fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
            total_processed: 0,
        }
    }

    /// Add a successful validation result
    pub fn add_success(&mut self, metadata: TemplateMetadata) {
        self.successful.push(metadata);
        self.total_processed += 1;
    }

    /// Add a failed validation result
    pub fn add_error(&mut self, path: PathBuf, error: ValidationError) {
        self.failed.push((path, error));
        self.total_processed += 1;
    }

    /// Get the number of successful validations
    pub fn success_count(&self) -> usize {
        self.successful.len()
    }

    /// Get the number of failed validations
    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }

    /// Check if all validations passed
    pub fn all_passed(&self) -> bool {
        self.failed.is_empty()
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_processed == 0 {
            100.0
        } else {
            (self.success_count() as f64 / self.total_processed as f64) * 100.0
        }
    }

    /// Get a summary string of the validation results
    pub fn summary(&self) -> String {
        format!(
            "Template Validation Report: {}/{} passed ({:.1}%)",
            self.success_count(),
            self.total_processed,
            self.success_rate()
        )
    }

    /// Display detailed report
    pub fn display_detailed(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("=== {} ===\n", self.summary()));

        if !self.successful.is_empty() {
            output.push_str("\n✅ Successful Templates:\n");
            for metadata in &self.successful {
                output.push_str(&format!(
                    "  - {} ({:?}, {} bytes, {} variables)\n",
                    metadata.name,
                    metadata.template_type,
                    metadata.size,
                    metadata.required_variables.len()
                ));
            }
        }

        if !self.failed.is_empty() {
            output.push_str("\n❌ Failed Templates:\n");
            for (path, error) in &self.failed {
                output.push_str(&format!("  - {}: {}\n", path.display(), error));
            }
        }

        output
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_template(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_template_type_detection() {
        assert_eq!(
            TemplateType::from_path(Path::new("test.rs")),
            TemplateType::Rust
        );
        assert_eq!(
            TemplateType::from_path(Path::new("test.toml")),
            TemplateType::Toml
        );
        assert_eq!(
            TemplateType::from_path(Path::new("test.html")),
            TemplateType::Html
        );
        assert_eq!(
            TemplateType::from_path(Path::new("handler.rs.txt")),
            TemplateType::Rust
        );
        assert_eq!(
            TemplateType::from_path(Path::new("unknown.xyz")),
            TemplateType::Unknown
        );
    }

    #[test]
    fn test_variable_extraction() {
        let validator = TemplateValidator::default();
        let content = r#"
        Hello {{ name }}!
        {% for item in items %}
            - {{ item.value }}
        {% endfor %}
        "#;

        let variables = validator.extract_template_variables(content);
        assert!(variables.contains(&"name".to_string()));
        assert!(variables.contains(&"items".to_string()));
        assert!(variables.contains(&"item.value".to_string()));
    }

    #[test]
    fn test_jinja2_syntax_validation() {
        let validator = TemplateValidator::default();

        // Valid syntax
        assert!(validator.validate_jinja2_syntax("{{ name }}").is_ok());
        assert!(validator
            .validate_jinja2_syntax("{% for item in items %}{% endfor %}")
            .is_ok());

        // Invalid syntax
        assert!(validator.validate_jinja2_syntax("{{ name }").is_err());
        assert!(validator
            .validate_jinja2_syntax("{{ {{ nested }} }}")
            .is_err());
        assert!(validator.validate_jinja2_syntax("{% unclosed").is_err());
    }

    #[test]
    fn test_template_validation() {
        let temp_dir = TempDir::new().unwrap();
        let template_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&template_dir).unwrap();

        // Create a valid Rust template
        let rust_template = r#"
// ⚠️  AUTO-GENERATED CODE - DO NOT EDIT
use serde::{Deserialize, Serialize};

pub struct {{ struct_name }} {
    pub name: String,
}
        "#;
        create_test_template(&template_dir, "handler.rs.txt", rust_template);

        // Create validator
        let config = ValidationConfig {
            template_dir: template_dir.clone(),
            ..Default::default()
        };
        let mut validator = TemplateValidator::new(config);

        // Validate the template
        let template_path = template_dir.join("handler.rs.txt");
        let result = validator.validate_template(&template_path);
        assert!(result.is_ok());

        let metadata = result.unwrap();
        assert_eq!(metadata.template_type, TemplateType::Rust);
        assert!(metadata
            .required_variables
            .contains(&"struct_name".to_string()));
    }

    #[test]
    fn test_validation_report() {
        let mut report = ValidationReport::new();
        assert_eq!(report.success_count(), 0);
        assert_eq!(report.failure_count(), 0);
        assert!(report.all_passed());

        let metadata = TemplateMetadata {
            path: PathBuf::from("test.rs"),
            name: "test.rs".to_string(),
            required_variables: vec!["name".to_string()],
            template_type: TemplateType::Rust,
            size: 100,
        };

        report.add_success(metadata);
        assert_eq!(report.success_count(), 1);
        assert_eq!(report.success_rate(), 100.0);

        report.add_error(
            PathBuf::from("bad.rs"),
            ValidationError::SyntaxError("Bad syntax".to_string()),
        );
        assert_eq!(report.failure_count(), 1);
        assert_eq!(report.success_rate(), 50.0);
        assert!(!report.all_passed());
    }
}
