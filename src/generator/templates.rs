use askama::Template;
// Remove explicit filters import; not needed and causes unresolved symbol errors
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use super::schema::{
    is_named_type, rust_literal_for_example, to_camel_case, FieldDef, TypeDefinition,
};
use crate::spec::{ParameterMeta, RouteMeta};

/// Entry in the handler registry for code generation
///
/// Contains all information needed to register a handler in the dispatcher.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    /// Handler function name
    pub name: String,
    /// Typed request struct name
    pub request_type: String,
    /// Controller struct name
    pub controller_struct: String,
    /// Route parameters
    pub parameters: Vec<ParameterMeta>,
}

/// Route information for display in generated code comments
#[derive(Debug, Clone)]
pub struct RouteDisplay {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Route path pattern
    pub path: String,
    /// Handler name
    pub handler: String,
}

/// Template data for generating Cargo.toml
#[derive(Template)]
#[template(path = "Cargo.toml.txt")]
pub struct CargoTomlTemplateData {
    /// Project name
    pub name: String,
}

/// Template for generating config.yaml with default settings
#[derive(Template)]
#[template(path = "config.yaml", escape = "none")]
pub struct ConfigYamlTemplate;

/// Template data for generating main.rs entry point
#[derive(Template)]
#[template(path = "main.rs.txt", escape = "none")]
pub struct MainRsTemplateData {
    /// Project name
    pub name: String,
    /// Routes for displaying in comments
    pub routes: Vec<RouteDisplay>,
}

/// Template for generating OpenAPI documentation HTML
#[derive(Template)]
#[template(path = "openapi.index.html", escape = "none")]
pub struct OpenapiIndexTemplate;

/// Template for generating static site index.html
#[derive(Template)]
#[template(path = "static.index.html", escape = "none")]
pub struct StaticIndexTemplate;

/// Template data for generating mod.rs module declarations
#[derive(Template)]
#[template(path = "mod.rs.txt")]
pub struct ModRsTemplateData {
    /// Module names to declare
    pub modules: Vec<String>,
}

/// Template data for generating registry.rs (handler registration)
#[derive(Template)]
#[template(path = "registry.rs.txt")]
pub struct RegistryTemplateData {
    /// Registry entries for all handlers
    pub entries: Vec<RegistryEntry>,
}

/// Template data for generating handler_types.rs (type definitions)
#[derive(Template)]
#[template(path = "handler_types.rs.txt")]
pub struct TypesTemplateData {
    /// Map of type names to definitions
    pub types: BTreeMap<String, TypeDefinition>,
}

/// Template data for generating a handler module
///
/// Contains all information needed to generate request/response types and a handler skeleton.
#[derive(Template)]
#[template(path = "handler.rs.txt")]
pub struct HandlerTemplateData {
    /// Handler function name
    pub handler_name: String,
    /// Request struct fields
    pub request_fields: Vec<FieldDef>,
    /// Response struct fields
    pub response_fields: Vec<FieldDef>,
    /// Whether the response is an array
    pub response_is_array: bool,
    /// Type of array elements (if response is array)
    pub response_array_type: String,
    /// Types to import (e.g., custom types from handler_types)
    pub imports: Vec<String>,
    /// Route parameters
    pub parameters: Vec<ParameterMeta>,
    /// Whether this handler uses Server-Sent Events
    pub sse: bool,
}

/// Template data for generating a controller module
///
/// Controllers spawn coroutines that dispatch requests to handlers.
#[derive(Template)]
#[template(path = "controller.rs.txt")]
pub struct ControllerTemplateData {
    /// Handler function name
    pub handler_name: String,
    /// Controller struct name
    pub struct_name: String,
    /// Response struct fields
    pub response_fields: Vec<FieldDef>,
    /// Example response as Rust code
    pub example: String,
    /// Whether an example response is available
    pub has_example: bool,
    /// Example response as JSON string
    pub example_json: String,
    /// Whether the response is an array
    pub response_is_array: bool,
    /// Array literal for response (if array)
    pub response_array_literal: String,
    /// Types to import
    pub imports: Vec<String>,
    /// Whether this handler uses Server-Sent Events
    pub sse: bool,
}

/// Write a handler module file
///
/// Generates a handler module with request/response types and a skeleton handler function.
///
/// # Arguments
///
/// * `path` - Output file path
/// * `handler` - Handler function name
/// * `req` - Request struct fields
/// * `res` - Response struct fields
/// * `imports` - Types to import
/// * `params` - Route parameters
/// * `sse` - Whether to use Server-Sent Events
/// * `force` - Overwrite existing file
///
/// # Errors
///
/// Returns an error if file writing fails
#[allow(clippy::too_many_arguments)]
pub fn write_handler(
    path: &Path,
    handler: &str,
    req: &[FieldDef],
    res: &[FieldDef],
    imports: &BTreeSet<String>,
    params: &[ParameterMeta],
    sse: bool,
    force: bool,
) -> anyhow::Result<()> {
    if path.exists() && !force {
        println!("⚠️  Skipping existing handler file: {path:?}");
        return Ok(());
    }
    let rendered = HandlerTemplateData {
        handler_name: handler.to_string(),
        request_fields: req.to_vec(),
        response_fields: res.to_vec(),
        response_is_array: res.len() == 1 && res[0].name == "items",
        response_array_type: res.first().map(|f| f.ty.clone()).unwrap_or_default(),
        imports: imports.iter().cloned().collect(),
        parameters: params.to_vec(),
        sse,
    }
    .render()?;
    fs::write(path, rendered)?;
    println!("✅ Generated handler: {path:?}");
    Ok(())
}

/// Write a controller module file
///
/// Generates a controller that spawns a coroutine to handle requests for a specific endpoint.
/// Controllers bridge the dispatcher and handlers.
///
/// # Arguments
///
/// * `path` - Output file path
/// * `handler` - Handler function name
/// * `struct_name` - Controller struct name
/// * `res` - Response struct fields
/// * `example` - Example response from OpenAPI spec
/// * `sse` - Whether to use Server-Sent Events
/// * `force` - Overwrite existing file
///
/// # Errors
///
/// Returns an error if file writing fails
pub fn write_controller(
    path: &Path,
    handler: &str,
    struct_name: &str,
    res: &[FieldDef],
    example: Option<Value>,
    sse: bool,
    force: bool,
) -> anyhow::Result<()> {
    if path.exists() && !force {
        println!("⚠️  Skipping existing controller file: {path:?}");
        return Ok(());
    }
    let example_map = example
        .as_ref()
        .and_then(|v| match v {
            Value::Object(map) => Some(map.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let enriched_fields = res
        .iter()
        .map(|field| {
            let value = example_map
                .get(&field.name)
                .map(|val| rust_literal_for_example(field, val))
                .unwrap_or_else(|| field.value.clone());
            FieldDef {
                name: field.name.clone(),
                original_name: field.original_name.clone(),
                ty: field.ty.clone(),
                optional: field.optional,
                value,
            }
        })
        .collect::<Vec<_>>();
    let mut imports = BTreeSet::new();
    for field in res {
        let inner = field
            .ty
            .strip_prefix("Vec<")
            .and_then(|s| s.strip_suffix(">"))
            .unwrap_or(&field.ty);
        if is_named_type(inner) {
            imports.insert(to_camel_case(inner));
        }
    }
    let example_pretty = example
        .as_ref()
        .and_then(|v| serde_json::to_string_pretty(v).ok())
        .unwrap_or_default();
    let example_json = if example_pretty.is_empty() {
        String::new()
    } else {
        example_pretty
            .lines()
            .map(|l| format!("        // {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let response_is_array = res.len() == 1 && res[0].name == "items";
    let array_literal = if response_is_array {
        // If the example itself is an array, prefer rendering from it
        if let Some(ref ex) = example {
            if ex.is_array() {
                let items_field = FieldDef {
                    name: "items".to_string(),
                    original_name: "items".to_string(),
                    ty: res[0].ty.clone(), // Vec<...>
                    optional: false,
                    value: String::new(),
                };
                super::schema::rust_literal_for_example(&items_field, ex)
            } else {
                enriched_fields
                    .first()
                    .map(|f| f.value.clone())
                    .unwrap_or_else(|| "vec![]".to_string())
            }
        } else {
            enriched_fields
                .first()
                .map(|f| f.value.clone())
                .unwrap_or_else(|| "vec![]".to_string())
        }
    } else {
        String::new()
    };
    let context = ControllerTemplateData {
        handler_name: handler.to_string(),
        struct_name: struct_name.to_string(),
        response_fields: enriched_fields.clone(),
        example: example_pretty,
        has_example: example.is_some(),
        example_json,
        response_is_array,
        response_array_literal: array_literal,
        imports: imports.iter().cloned().collect(),
        sse,
    };
    fs::write(path, context.render()?)?;
    println!("✅ Generated controller: {path:?}");
    Ok(())
}

pub(crate) fn write_mod_rs(dir: &Path, modules: &[String], label: &str) -> anyhow::Result<()> {
    let path = dir.join("mod.rs");
    let rendered = ModRsTemplateData {
        modules: modules.to_vec(),
    }
    .render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Updated mod.rs for {label} → {path:?}");
    Ok(())
}

/// Write the registry.rs file
///
/// Generates the handler registry that registers all handlers with the dispatcher.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `src/`)
/// * `entries` - Registry entries for all handlers
///
/// # Errors
///
/// Returns an error if file writing fails
pub fn write_registry_rs(dir: &Path, entries: &[RegistryEntry]) -> anyhow::Result<()> {
    let path = dir.join("registry.rs");
    let rendered = RegistryTemplateData {
        entries: entries.to_vec(),
    }
    .render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Generated registry.rs → {path:?}");
    Ok(())
}

pub(crate) fn write_types_rs(
    dir: &Path,
    types: &HashMap<String, TypeDefinition>,
) -> anyhow::Result<()> {
    let path = dir.join("types.rs");
    let mut sorted = BTreeMap::new();
    for (name, def) in types {
        sorted.insert(name.clone(), def.clone());
    }
    let rendered = TypesTemplateData { types: sorted }.render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Generated types.rs → {path:?}");
    Ok(())
}

pub(crate) fn write_cargo_toml(base: &Path, slug: &str) -> anyhow::Result<()> {
    let rendered = CargoTomlTemplateData {
        name: slug.to_string(),
    }
    .render()?;
    fs::write(base.join("Cargo.toml"), rendered)?;
    println!("✅ Wrote Cargo.toml");
    Ok(())
}

/// Write the main.rs entry point
///
/// Generates the main.rs file that starts the HTTP server and registers handlers.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `src/`)
/// * `slug` - Project name slug
/// * `routes` - All routes from the OpenAPI spec
///
/// # Errors
///
/// Returns an error if file writing fails
pub fn write_main_rs(dir: &Path, slug: &str, routes: Vec<RouteMeta>) -> anyhow::Result<()> {
    let routes = routes
        .into_iter()
        .map(|r| RouteDisplay {
            method: r.method.to_string(),
            path: r.path_pattern,
            handler: r.handler_name,
        })
        .collect();
    let rendered = MainRsTemplateData {
        name: slug.to_string(),
        routes,
    }
    .render()?;
    fs::write(dir.join("main.rs"), rendered)?;
    println!("✅ Wrote main.rs");
    Ok(())
}

/// Write the OpenAPI documentation index.html
///
/// Generates an HTML page that displays the OpenAPI specification using Swagger UI.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `doc/`)
///
/// # Errors
///
/// Returns an error if file writing fails
pub fn write_openapi_index(dir: &Path) -> anyhow::Result<()> {
    let rendered = OpenapiIndexTemplate.render()?;
    fs::write(dir.join("index.html"), rendered)?;
    println!("✅ Wrote docs index → {:?}", dir.join("index.html"));
    Ok(())
}

/// Write the static site index.html
///
/// Generates a simple placeholder index page for the static site.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `static_site/`)
///
/// # Errors
///
/// Returns an error if file writing fails
pub fn write_static_index(dir: &Path) -> anyhow::Result<()> {
    let rendered = StaticIndexTemplate.render()?;
    fs::write(dir.join("index.html"), rendered)?;
    println!("✅ Wrote static index → {:?}", dir.join("index.html"));
    Ok(())
}

/// Write the default config.yaml
///
/// Generates a configuration file with default settings for the application.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `config/`)
///
/// # Errors
///
/// Returns an error if file writing fails
pub fn write_default_config(dir: &Path) -> anyhow::Result<()> {
    let rendered = ConfigYamlTemplate.render()?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("config.yaml"), rendered)?;
    println!("✅ Wrote default config → {:?}", dir.join("config.yaml"));
    Ok(())
}
