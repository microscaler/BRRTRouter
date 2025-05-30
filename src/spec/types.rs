use super::SecurityRequirement;
use http::Method;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterStyle {
    Matrix,
    Label,
    Form,
    Simple,
    SpaceDelimited,
    PipeDelimited,
    DeepObject,
}

impl From<oas3::spec::ParameterStyle> for ParameterStyle {
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

impl std::fmt::Display for ParameterStyle {
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
        write!(f, "{}", s)
    }
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

impl RouteMeta {
    pub fn content_type_for(&self, status: u16) -> Option<String> {
        self.responses
            .get(&status)
            .and_then(|m| m.keys().next())
            .cloned()
    }
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub schema: Option<Value>,
    pub style: Option<ParameterStyle>,
    pub explode: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResponseSpec {
    pub schema: Option<Value>,
    pub example: Option<Value>,
}

pub type Responses =
    std::collections::HashMap<u16, std::collections::HashMap<String, ResponseSpec>>;
