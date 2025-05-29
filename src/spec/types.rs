use http::Method;
use serde_json::Value;
use std::path::PathBuf;
use super::SecurityRequirement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

impl std::fmt::Display for ParameterLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterLocation::Path => write!(f, "Path"),
            ParameterLocation::Query => write!(f, "Query"),
            ParameterLocation::Header => write!(f, "Header"),
            ParameterLocation::Cookie => write!(f, "Cookie"),
        }
    }
}

impl From<oas3::spec::ParameterIn> for ParameterLocation {
    fn from(loc: oas3::spec::ParameterIn) -> Self {
        match loc {
            oas3::spec::ParameterIn::Path => ParameterLocation::Path,
            oas3::spec::ParameterIn::Query => ParameterLocation::Query,
            oas3::spec::ParameterIn::Header => ParameterLocation::Header,
            oas3::spec::ParameterIn::Cookie => ParameterLocation::Cookie,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteMeta {
    pub method: Method,
    pub path_pattern: String,
    pub handler_name: String,
    pub parameters: Vec<ParameterMeta>,
    pub request_schema: Option<Value>,
    pub response_schema: Option<Value>,
    pub example: Option<Value>,
    pub responses: Responses,
    pub security: Vec<SecurityRequirement>,
    pub example_name: String,
    pub project_slug: String,
    pub output_dir: PathBuf,
    pub base_path: String,
    pub sse: bool,
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResponseSpec {
    pub schema: Option<Value>,
    pub example: Option<Value>,
}

pub type Responses = std::collections::HashMap<u16, std::collections::HashMap<String, ResponseSpec>>;
