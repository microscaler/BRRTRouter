use askama::Template;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use super::schema::{
    build_complete_example_object, is_named_type, rust_literal_for_example, to_camel_case, FieldDef, TypeDefinition,
};
use crate::spec::{ParameterMeta, RouteMeta};

#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub name: String,
    pub request_type: String,
    pub controller_struct: String,
    pub parameters: Vec<ParameterMeta>,
}

#[derive(Debug, Clone)]
pub struct RouteDisplay {
    pub method: String,
    pub path: String,
    pub handler: String,
}

#[derive(Template)]
#[template(path = "Cargo.toml.txt")]
pub struct CargoTomlTemplateData {
    pub name: String,
}

#[derive(Template)]
#[template(path = "main.rs.txt", escape = "none")]
pub struct MainRsTemplateData {
    pub name: String,
    pub routes: Vec<RouteDisplay>,
}

#[derive(Template)]
#[template(path = "openapi.index.html", escape = "none")]
pub struct OpenapiIndexTemplate;

#[derive(Template)]
#[template(path = "static.index.html", escape = "none")]
pub struct StaticIndexTemplate;

#[derive(Template)]
#[template(path = "mod.rs.txt")]
pub struct ModRsTemplateData {
    pub modules: Vec<String>,
}

#[derive(Template)]
#[template(path = "registry.rs.txt")]
pub struct RegistryTemplateData {
    pub entries: Vec<RegistryEntry>,
}

// Temporarily disabled to focus on controller debugging
// #[derive(Template)]
// #[template(path = "handler.rs.txt")]
// pub struct HandlerTemplateData {
//     pub handler_name: String,
//     pub request_fields: Vec<FieldDef>,
//     pub response_fields: Vec<FieldDef>,
//     pub response_is_array: bool,
//     pub response_array_type: String,
//     pub imports: Vec<String>,
//     pub parameters: Vec<ParameterMeta>,
//     pub sse: bool,
//     pub spec_path: String,
//     pub generation_time: String,
// }

#[derive(Template)]
#[template(path = "controllers/controller.rs.txt")]
pub struct ControllerTemplateData {
    pub handler_name: String,
    pub struct_name: String,
    pub response_fields: Vec<FieldDef>,
    pub example: String,
    pub has_example: bool,
    pub example_json: String,
    pub response_is_array: bool,
    pub response_array_literal: String,
    pub imports: Vec<String>,
    pub sse: bool,
    pub spec_path: String,
    pub generation_time: String,
}

pub(crate) fn write_handler(
    _handler_dir: &Path,
    _operation_id: &str,
    _parameters: &[ParameterMeta],
    _request_fields: Vec<FieldDef>,
    _response_fields: Vec<FieldDef>,
    _has_request_body: bool,
    _response_is_array: bool,
    _response_array_type: String,
    _sse: bool,
    _force: bool,
) -> anyhow::Result<()> {
    // Temporarily disabled to fix compilation
    println!("⚠️ Handler generation temporarily disabled");
    Ok(())
}

pub fn write_controller(
    path: &Path,
    handler: &str,
    struct_name: &str,
    res: &[FieldDef],
    example: Option<Value>,
    sse: bool,
    force: bool,
    schema_types: &HashMap<String, TypeDefinition>,
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
            let value = if is_named_type(&field.ty) {
                // For complex named types, try to get the complete schema and build a full example
                if let Some(type_def) = schema_types.get(&field.ty) {
                    // Use the original OpenAPI schema if available
                    if let Some(original_schema) = &type_def.original_schema {
                        let complete_example = build_complete_example_object(original_schema);
                        rust_literal_for_example(field, &complete_example)
                    } else {
                        // Fall back to reconstructed schema
                        let schema_json = build_complete_example_object(&type_definition_to_schema(type_def));
                        rust_literal_for_example(field, &schema_json)
                    }
                } else {
                    // Fall back to existing example or field value
                    example_map
                        .get(&field.name)
                        .map(|val| rust_literal_for_example(field, val))
                        .unwrap_or_else(|| field.value.clone())
                }
            } else if field.ty.starts_with("Vec<") {
                // Handle arrays of complex types
                if let Some(inner_ty) = field.ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix(">")) {
                    if is_named_type(inner_ty) && schema_types.contains_key(inner_ty) {
                        // Create an array with multiple complete example objects
                        if let Some(type_def) = schema_types.get(inner_ty) {
                            let fallback_schema;
                            let schema_to_use = if let Some(original_schema) = &type_def.original_schema {
                                println!("🔍 TEMPLATE DEBUG: Using original schema for {}", inner_ty);
                                original_schema
                            } else {
                                println!("🔍 TEMPLATE DEBUG: Using fallback schema for {}", inner_ty);
                                fallback_schema = type_definition_to_schema(type_def);
                                &fallback_schema
                            };
                            
                            println!("🔍 TEMPLATE DEBUG: Schema for {} array generation: {}", inner_ty, serde_json::to_string_pretty(schema_to_use).unwrap_or("invalid".to_string()));
                            
                            let schema_json1 = build_complete_example_object(schema_to_use);
                            let schema_json2 = build_complete_example_object(schema_to_use);
                            let example1 = rust_literal_for_example(&FieldDef {
                                name: "temp".to_string(),
                                ty: inner_ty.to_string(),
                                optional: false,
                                value: String::new(),
                                documentation: None,
                                validation_attrs: None,
                            }, &schema_json1);
                            let example2 = rust_literal_for_example(&FieldDef {
                                name: "temp".to_string(), 
                                ty: inner_ty.to_string(),
                                optional: false,
                                value: String::new(),
                                documentation: None,
                                validation_attrs: None,
                            }, &schema_json2);
                            format!("vec![{example1}, {example2}]")
                        } else {
                            field.value.clone()
                        }
                    } else {
                        example_map
                            .get(&field.name)
                            .map(|val| rust_literal_for_example(field, val))
                            .unwrap_or_else(|| field.value.clone())
                    }
                } else {
                    field.value.clone()
                }
            } else {
                // For primitive types, use example or field value
                example_map
                    .get(&field.name)
                    .map(|val| rust_literal_for_example(field, val))
                    .unwrap_or_else(|| field.value.clone())
            };
            
            FieldDef {
                name: field.name.clone(),
                ty: field.ty.clone(),
                optional: field.optional,
                value,
                documentation: field.documentation.clone(),
                validation_attrs: field.validation_attrs.clone(),
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
        // For array responses, use the actual OpenAPI example data if available
        example
            .as_ref()
            .and_then(|v| match v {
                Value::Array(_arr) => {
                    // Convert the example array to Rust literal
                    let field = &res[0]; // This is the "items" field
                    Some(rust_literal_for_example(field, v))
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                // Fallback to enriched field value if no example array
                enriched_fields
                    .first()
                    .map(|f| f.value.clone())
                    .unwrap_or_else(|| "vec![Default::default()]".to_string())
            })
    } else {
        // For non-array responses, use the enriched field value
        enriched_fields
            .first()
            .map(|f| f.value.clone())
            .unwrap_or_else(|| "vec![Default::default()]".to_string())
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
        spec_path: "OpenAPI specification".to_string(),
        generation_time: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    };
    fs::write(path, context.render()?)?;
    println!("✅ Generated controller: {path:?}");
    Ok(())
}

/// Convert a TypeDefinition to a schema Value for building examples
fn type_definition_to_schema(type_def: &TypeDefinition) -> Value {
    let mut schema = serde_json::Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    
    for field in &type_def.fields {
        // Create a property schema for each field
        let mut prop = serde_json::Map::new();
        
        // Set the type based on the Rust type
        let (json_type, format, example): (&str, Option<&str>, Option<Value>) = match field.ty.as_str() {
            "String" => ("string", None, Some(Value::String("example".to_string()))),
            "i32" | "i64" => ("integer", None, Some(Value::Number(serde_json::Number::from(42)))),
            "f32" | "f64" => ("number", None, Some(Value::Number(serde_json::Number::from_f64(42.0).unwrap()))),
            "bool" => ("boolean", None, Some(Value::Bool(true))),
            _ if field.ty.starts_with("Vec<") => {
                prop.insert("type".to_string(), Value::String("array".to_string()));
                properties.insert(field.name.clone(), Value::Object(prop));
                if !field.optional {
                    required.push(Value::String(field.name.clone()));
                }
                continue;
            }
            _ => ("string", None, Some(Value::String("example".to_string()))),
        };
        
        prop.insert("type".to_string(), Value::String(json_type.to_string()));
        if let Some(fmt) = format {
            prop.insert("format".to_string(), Value::String(fmt.to_string()));
        }
        if let Some(ex) = example {
            prop.insert("example".to_string(), ex);
        }
        
        properties.insert(field.name.clone(), Value::Object(prop));
        
        if !field.optional {
            required.push(Value::String(field.name.clone()));
        }
    }
    
    schema.insert("properties".to_string(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    
    Value::Object(schema)
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

pub(crate) fn write_cargo_toml(base: &Path, slug: &str) -> anyhow::Result<()> {
    let rendered = CargoTomlTemplateData {
        name: slug.to_string(),
    }
    .render()?;
    fs::write(base.join("Cargo.toml"), rendered)?;
    println!("✅ Wrote Cargo.toml");
    Ok(())
}

pub fn write_main_rs(
    base_dir: &Path,
    project_name: &str,
    routes: &[crate::spec::RouteMeta],
    _force: bool,
) -> anyhow::Result<()> {
    let routes = routes
        .iter()
        .map(|r| RouteDisplay {
            method: r.method.to_string(),
            path: r.path_pattern.clone(),
            handler: r.handler_name.clone(),
        })
        .collect();
    let rendered = MainRsTemplateData {
        name: project_name.to_string(),
        routes,
    }
    .render()?;
    fs::write(base_dir.join("main.rs"), rendered)?;
    println!("✅ Wrote main.rs");
    Ok(())
}

pub fn write_openapi_index(dir: &Path) -> anyhow::Result<()> {
    let rendered = OpenapiIndexTemplate.render()?;
    fs::write(dir.join("index.html"), rendered)?;
    println!("✅ Wrote docs index → {:?}", dir.join("index.html"));
    Ok(())
}

pub fn write_static_index(dir: &Path) -> anyhow::Result<()> {
    let rendered = StaticIndexTemplate.render()?;
    fs::write(dir.join("index.html"), rendered)?;
    println!("✅ Wrote static index → {:?}", dir.join("index.html"));
    Ok(())
}
