use super::SecurityRequirement;
use http::Method;
use serde_json::Value;
use std::path::PathBuf;

/// Location where a parameter can be found in an HTTP request
///
/// Corresponds to the OpenAPI `in` field for parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterLocation {
    /// Path parameter (e.g., `/users/{id}`)
    Path,
    /// Query string parameter (e.g., `?limit=10`)
    Query,
    /// HTTP header parameter
    Header,
    /// Cookie parameter
    Cookie,
}

/// Serialization style for parameters as defined by OpenAPI
///
/// Determines how arrays and objects are serialized in different parameter locations.
/// See: https://spec.openapis.org/oas/v3.1.0#style-values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterStyle {
    /// Path-style parameters (e.g., `;color=blue;color=green`)
    Matrix,
    /// Label-style parameters with dot prefix (e.g., `.blue.green`)
    Label,
    /// Form-style parameters (default for query, e.g., `color=blue&color=green`)
    Form,
    /// Simple-style parameters (default for path/header, e.g., `blue,green`)
    Simple,
    /// Space-delimited parameters (e.g., `blue green`)
    SpaceDelimited,
    /// Pipe-delimited parameters (e.g., `blue|green`)
    PipeDelimited,
    /// Deep object parameters for complex objects (e.g., `color[R]=100&color[G]=200`)
    DeepObject,
}

/// Convert from `oas3` crate's `ParameterStyle` to BRRTRouter's enum
///
/// Maps OpenAPI 3.x parameter style definitions from the `oas3` parser
/// to BRRTRouter's internal representation.
///
/// # OpenAPI Parameter Styles
///
/// Different encoding styles for parameter values:
/// - **Simple**: Comma-separated (default for path/header)
/// - **Form**: Ampersand-separated (default for query/cookie)
/// - **Matrix**: Semicolon-prefixed (path only)
/// - **Label**: Dot-prefixed (path only)
/// - **SpaceDelimited**: Space-separated arrays
/// - **PipeDelimited**: Pipe-separated arrays
/// - **DeepObject**: Nested objects as `key[subkey]=value`
impl From<oas3::spec::ParameterStyle> for ParameterStyle {
    /// Convert `oas3::spec::ParameterStyle` to `ParameterStyle`
    ///
    /// # Arguments
    ///
    /// * `style` - Parameter style from parsed OpenAPI spec
    ///
    /// # Returns
    ///
    /// Equivalent BRRTRouter `ParameterStyle` enum variant
    fn from(style: oas3::spec::ParameterStyle) -> Self {
        use oas3::spec::ParameterStyle as PS;
        match style {
            PS::Matrix => ParameterStyle::Matrix,
            PS::Label => ParameterStyle::Label,
            PS::Form => ParameterStyle::Form,
            PS::Simple => ParameterStyle::Simple,
            PS::SpaceDelimited => ParameterStyle::SpaceDelimited,
            PS::PipeDelimited => ParameterStyle::PipeDelimited,
            PS::DeepObject => ParameterStyle::DeepObject,
        }
    }
}

/// Display implementation for `ParameterStyle`
///
/// Formats the parameter style as a human-readable string for logging,
/// error messages, and debugging output.
///
/// # Output Format
///
/// Returns the enum variant name as-is: "Matrix", "Label", "Form", etc.
///
/// # Usage
///
/// ```rust
/// use brrtrouter::spec::ParameterStyle;
///
/// let style = ParameterStyle::Form;
/// println!("Style: {}", style); // "Style: Form"
/// ```
impl std::fmt::Display for ParameterStyle {
    /// Format the parameter style as a string
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter to write the output
    ///
    /// # Returns
    ///
    /// `Ok(())` if formatting succeeds, `Err` otherwise
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ParameterStyle::Matrix => "Matrix",
            ParameterStyle::Label => "Label",
            ParameterStyle::Form => "Form",
            ParameterStyle::Simple => "Simple",
            ParameterStyle::SpaceDelimited => "SpaceDelimited",
            ParameterStyle::PipeDelimited => "PipeDelimited",
            ParameterStyle::DeepObject => "DeepObject",
        };
        write!(f, "{s}")
    }
}

/// Display implementation for `ParameterLocation`
///
/// Formats the parameter location as a human-readable string for logging,
/// error messages, and validation output.
///
/// # Output Format
///
/// Returns the enum variant name: "Path", "Query", "Header", "Cookie"
///
/// # Usage
///
/// ```rust
/// use brrtrouter::spec::ParameterLocation;
///
/// let loc = ParameterLocation::Query;
/// println!("Location: {}", loc); // "Location: Query"
/// ```
impl std::fmt::Display for ParameterLocation {
    /// Format the parameter location as a string
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter to write the output
    ///
    /// # Returns
    ///
    /// `Ok(())` if formatting succeeds, `Err` otherwise
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterLocation::Path => write!(f, "Path"),
            ParameterLocation::Query => write!(f, "Query"),
            ParameterLocation::Header => write!(f, "Header"),
            ParameterLocation::Cookie => write!(f, "Cookie"),
        }
    }
}

/// Convert from `oas3` crate's `ParameterIn` to BRRTRouter's enum
///
/// Maps OpenAPI 3.x parameter location definitions from the `oas3` parser
/// to BRRTRouter's internal representation.
///
/// # OpenAPI Parameter Locations
///
/// Parameters can appear in different parts of the request:
/// - **Path**: URL path segments (`/users/{id}`)
/// - **Query**: URL query string (`/users?id=123`)
/// - **Header**: HTTP headers (`Authorization: Bearer token`)
/// - **Cookie**: HTTP cookies (`session=abc123`)
///
/// # Usage
///
/// ```rust
/// use brrtrouter::spec::ParameterLocation;
///
/// let oas_param = oas3::spec::ParameterIn::Query;
/// let loc: ParameterLocation = oas_param.into();
/// ```
impl From<oas3::spec::ParameterIn> for ParameterLocation {
    /// Convert `oas3::spec::ParameterIn` to `ParameterLocation`
    ///
    /// # Arguments
    ///
    /// * `loc` - Parameter location from parsed OpenAPI spec
    ///
    /// # Returns
    ///
    /// Equivalent BRRTRouter `ParameterLocation` enum variant
    fn from(loc: oas3::spec::ParameterIn) -> Self {
        match loc {
            oas3::spec::ParameterIn::Path => ParameterLocation::Path,
            oas3::spec::ParameterIn::Query => ParameterLocation::Query,
            oas3::spec::ParameterIn::Header => ParameterLocation::Header,
            oas3::spec::ParameterIn::Cookie => ParameterLocation::Cookie,
        }
    }
}

/// Metadata for a single API route derived from an OpenAPI operation
///
/// Contains all information needed to generate handlers, validate requests/responses,
/// and register the route with the dispatcher.
#[derive(Debug, Clone)]
pub struct RouteMeta {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: Method,
    /// Path pattern with parameter placeholders (e.g., `/users/{id}`)
    pub path_pattern: String,
    /// Generated handler function name
    pub handler_name: String,
    /// Parameters extracted from path, query, headers, and cookies
    pub parameters: Vec<ParameterMeta>,
    /// JSON Schema for request body validation
    pub request_schema: Option<Value>,
    /// Whether the request body is required
    pub request_body_required: bool,
    /// JSON Schema for response body validation
    pub response_schema: Option<Value>,
    /// Example response data from OpenAPI spec
    pub example: Option<Value>,
    /// All possible responses by status code and content type
    pub responses: Responses,
    /// Security requirements for this route (API keys, JWT, OAuth2, etc.)
    pub security: Vec<SecurityRequirement>,
    /// Example name used in generated code
    pub example_name: String,
    /// Project slug for file naming
    pub project_slug: String,
    /// Output directory for generated files
    pub output_dir: PathBuf,
    /// Base path prefix for the API
    pub base_path: String,
    /// Whether this route uses Server-Sent Events
    pub sse: bool,
    /// Estimated request body size in bytes (derived from OpenAPI schema)
    /// Used as fallback when Content-Length header is not available
    pub estimated_request_body_bytes: Option<usize>,
    /// Vendor extension override for stack size (x-brrtrouter-stack-size)
    pub x_brrtrouter_stack_size: Option<usize>,
}

impl RouteMeta {
    /// Get the content type for a specific HTTP status code response
    ///
    /// Returns the first content type defined for the given status code
    /// (typically `application/json`).
    pub fn content_type_for(&self, status: u16) -> Option<String> {
        self.responses
            .get(&status)
            .and_then(|m| m.keys().next())
            .cloned()
    }
}

/// Metadata for a single parameter in an API route
///
/// Extracted from OpenAPI parameter definitions and used for validation
/// and type generation.
#[derive(Debug, Clone)]
pub struct ParameterMeta {
    /// Parameter name
    pub name: String,
    /// Where the parameter appears in the request
    pub location: ParameterLocation,
    /// Whether the parameter is required
    pub required: bool,
    /// JSON Schema for parameter validation
    pub schema: Option<Value>,
    /// Serialization style (how arrays/objects are encoded)
    pub style: Option<ParameterStyle>,
    /// Whether to use exploded format for arrays/objects
    pub explode: Option<bool>,
}

/// Specification for a single response variant
///
/// Contains schema and example data for a specific HTTP status code and content type.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseSpec {
    /// JSON Schema for response body validation
    pub schema: Option<Value>,
    /// Example response data from OpenAPI spec
    pub example: Option<Value>,
}

/// Map of HTTP status codes to content types to response specifications
///
/// Example: `{ 200: { "application/json": ResponseSpec { ... } } }`
pub type Responses =
    std::collections::HashMap<u16, std::collections::HashMap<String, ResponseSpec>>;
