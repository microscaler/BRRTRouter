use crate::validator::{fail_if_issues, ValidationIssue};
use http::Method;
use oas3::spec::{ObjectOrReference, Parameter};
use oas3::OpenApiV3Spec;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "brrrouter")]
#[command(about = "BRRTRouter CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate handler stubs from an OpenAPI spec
    Generate {
        #[command(subcommand)]
        sub: GenerateSubcommand,
    },
}

#[derive(Subcommand)]
pub enum GenerateSubcommand {
    /// Generate handler files from OpenAPI spec
    Handlers {
        #[arg(short, long)]
        spec: PathBuf,

        #[arg(short, long, default_value = "src/handlers")]
        out: PathBuf,

        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
}


#[derive(Debug, Clone)]
pub struct RouteMeta {
    pub method: Method,
    pub path_pattern: String,
    pub handler_name: String,
    pub parameters: Vec<ParameterMeta>,
    pub request_schema: Option<Value>,
    pub response_schema: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub name: String,
    pub location: String,
    pub required: bool,
    pub schema: Option<Value>,
}

pub fn run_cli() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Generate { sub } => match sub {
            GenerateSubcommand::Handlers { spec, out, force } => {
                crate::generator::generate_handlers_from_spec(spec, out, *force)
                    .expect("failed to generate handlers");
            }
        },
    }
}

pub fn load_spec(file_path: &str, verbose: bool) -> anyhow::Result<Vec<RouteMeta>> {
    let content = std::fs::read_to_string(file_path)?;
    let spec: OpenApiV3Spec = if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
        serde_yaml::from_str(&content)?
    } else {
        serde_json::from_str(&content)?
    };

    build_routes(&spec, verbose)
}

fn build_routes(spec: &OpenApiV3Spec, verbose: bool) -> anyhow::Result<Vec<RouteMeta>> {
    let mut routes = Vec::new();
    let mut issues = Vec::new();

    if let Some(paths_map) = spec.paths.as_ref() {
        for (path, item) in paths_map {
            for (method_str, operation) in item.methods() {
                let method = method_str.clone();
                let location = format!("{} → {}", path, method);

                let handler_name = operation.extensions.iter().find_map(|(key, val)| {
                    if key.starts_with("handler") {
                        match val {
                            Value::String(s) => Some(s.clone()),
                            _ => None,
                        }
                    } else {
                        None
                    }
                });

                let handler_name = match handler_name {
                    Some(name) => name,
                    None => {
                        issues.push(ValidationIssue::new(
                            &location,
                            "MissingHandler",
                            "Missing x-handler-* extension",
                        ));
                        continue;
                    }
                };

                let request_schema = operation.request_body.as_ref().and_then(|r| match r {
                    ObjectOrReference::Object(req_body) => req_body
                        .content
                        .get("application/json")
                        .and_then(|media| media.schema.as_ref())
                        .and_then(|sref| match sref {
                            ObjectOrReference::Object(schema_obj) => {
                                let schema_val = serde_json::to_value(schema_obj).ok();
                                println!(
                                    "🔍 Parsed request schema for {}: {:#?}",
                                    handler_name, schema_val
                                );
                                schema_val
                            }
                            _ => None,
                        }),
                    _ => None,
                });

                let response_schema = operation
                    .responses
                    .as_ref()
                    .and_then(|responses_map| responses_map.get("200"))
                    .and_then(|resp| match resp {
                        ObjectOrReference::Object(resp_obj) => resp_obj
                            .content
                            .get("application/json")
                            .and_then(|media| media.schema.as_ref())
                            .and_then(|sref| match sref {
                                ObjectOrReference::Object(schema_obj) => {
                                    let schema_val = serde_json::to_value(schema_obj).ok();
                                    println!(
                                        "🔍 Parsed response schema for {}: {:#?}",
                                        handler_name, schema_val
                                    );
                                    schema_val
                                }
                                _ => None,
                            }),
                        _ => None,
                    });

                routes.push(RouteMeta {
                    method,
                    path_pattern: path.clone(),
                    handler_name,
                    parameters: vec![],
                    request_schema,
                    response_schema,
                });
            }
        }
    }

    fail_if_issues(issues);
    Ok(routes)
}
