// generator.rs
use crate::dummy_value;
use crate::spec::{build_routes, load_spec, resolve_schema_ref};
use askama::Template;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Template)]
#[template(path = "handler.rs.txt")]
pub struct HandlerTemplateData {
    pub handler_name: String,
    pub request_fields: Vec<FieldDef>,
    pub response_fields: Vec<FieldDef>,
    pub imports: Vec<String>,
}

#[derive(Template)]
#[template(path = "controller.rs.txt")]
pub struct ControllerTemplateData {
    pub handler_name: String,
    pub response_fields: Vec<FieldDef>,
    pub example: String,
    pub has_example: bool,
    pub example_json: String,
}

#[derive(Template)]
#[template(path = "registry.rs.txt")]
pub struct RegistryTemplateData {
    pub entries: Vec<RegistryEntry>,
}

#[derive(Template)]
#[template(path = "handler_types.rs.txt")]
pub struct TypesTemplateData {
    pub types: HashMap<String, TypeDefinition>,
}

#[derive(Template)]
#[template(path = "mod.rs.txt")]
pub struct ModRsTemplateData {
    pub modules: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: String,
    pub optional: bool,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub name: String,
}

pub fn generate_handlers_from_spec(
    spec_path: &Path,
    out_dir: &Path,
    force: bool,
) -> anyhow::Result<()> {
    println!("📦 Generating handlers from spec: {:?}", spec_path);

    let spec_str = std::fs::read_to_string(spec_path)?;
    let spec: oas3::OpenApiV3Spec = if spec_path.extension().map(|s| s == "yaml").unwrap_or(false) {
        serde_yaml::from_str(&spec_str)?
    } else {
        serde_json::from_str(&spec_str)?
    };
    let routes = build_routes(&spec, false)?;

    let mut schema_types = collect_component_schemas(&spec);

    fs::create_dir_all(out_dir)?;
    let controller_dir = Path::new("src/controllers");
    fs::create_dir_all(controller_dir)?;

    let mut seen = HashSet::new();
    let mut modules_handlers = Vec::new();
    let mut modules_controllers = Vec::new();
    let mut registry_entries = Vec::new();

    for route in routes {
        let handler = route.handler_name.clone();
        if !seen.insert(handler.clone()) {
            continue;
        }

        let request_fields = route.request_schema.as_ref().map_or(vec![], extract_fields);
        let response_fields = route
            .response_schema
            .as_ref()
            .map_or(vec![], extract_fields);

        let mut imports = BTreeSet::new();
        for field in request_fields.iter().chain(response_fields.iter()) {
            let inner = field
                .ty
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix(">"))
                .unwrap_or(&field.ty);

            if is_named_type(inner) {
                imports.insert(to_camel_case(inner));
            }
        }

        generate_handler_file(
            out_dir,
            &handler,
            &request_fields,
            &response_fields,
            &imports,
            force,
        )?;
        generate_controller_file(
            controller_dir,
            &handler,
            &response_fields,
            force,
            route.example.clone(),
        )?;

        modules_handlers.push(handler.clone());
        modules_controllers.push(handler.clone());
        registry_entries.push(RegistryEntry {
            name: handler.clone(),
        });

        if let Some(schema) = &route.request_schema {
            let name = format!("{}Request", handler);
            process_schema_type(&name, schema, &mut schema_types);
        }
        if let Some(schema) = &route.response_schema {
            let name = format!("{}Response", handler);
            process_schema_type(&name, schema, &mut schema_types);
        }
    }

    write_mod_rs_for_handlers(out_dir, modules_handlers)?;
    write_mod_rs_for_controllers(controller_dir, &modules_controllers)?;
    write_registry_rs(&registry_entries)?;
    write_types_rs(out_dir, &schema_types)?;

    Ok(())
}

fn write_mod_rs_for_handlers(dir: &Path, mut modules: Vec<String>) -> anyhow::Result<()> {
    if !modules.iter().any(|m| m == "types") {
        modules.insert(0, "types".to_string());
    }
    write_mod_rs(dir, &modules, "handlers")
}

fn write_mod_rs_for_controllers(dir: &Path, modules: &[String]) -> anyhow::Result<()> {
    write_mod_rs(dir, modules, "controllers")
}

fn write_mod_rs(dir: &Path, modules: &[String], label: &str) -> anyhow::Result<()> {
    let mod_rs_path = dir.join("mod.rs");
    let context = ModRsTemplateData {
        modules: modules.to_vec(),
    };
    let rendered = context.render()?;
    fs::write(&mod_rs_path, rendered)?;
    println!("✅ Updated mod.rs for {} → {:?}", label, mod_rs_path);
    Ok(())
}

fn write_registry_rs(entries: &[RegistryEntry]) -> anyhow::Result<()> {
    let registry_path = Path::new("src/registry.rs");
    let context = RegistryTemplateData {
        entries: entries.to_vec(),
    };
    let rendered = context.render()?;
    fs::write(&registry_path, rendered)?;
    println!("✅ Generated registry.rs → {:?}", registry_path);
    Ok(())
}

fn write_types_rs(
    out_dir: &Path,
    schema_types: &HashMap<String, TypeDefinition>,
) -> anyhow::Result<()> {
    let types_rs_path = out_dir.join("types.rs");
    let types_context = TypesTemplateData {
        types: schema_types.clone(),
    };
    let rendered = types_context.render()?;
    fs::write(&types_rs_path, rendered)?;
    println!("✅ Generated types.rs → {:?}", types_rs_path);
    Ok(())
}

fn generate_handler_file(
    out_dir: &Path,
    handler: &str,
    request_fields: &[FieldDef],
    response_fields: &[FieldDef],
    imports: &BTreeSet<String>,
    force: bool,
) -> anyhow::Result<()> {
    let file_path = out_dir.join(format!("{}.rs", handler));
    if file_path.exists() && !force {
        println!("⚠️  Skipping existing handler file: {:?}", file_path);
        return Ok(());
    }
    let context = HandlerTemplateData {
        handler_name: handler.to_string(),
        request_fields: request_fields.to_vec(),
        response_fields: response_fields.to_vec(),
        imports: imports.iter().cloned().collect(),
    };
    let rendered = context.render()?;
    fs::write(&file_path, rendered)?;
    println!("✅ Generated handler: {} → {:?}", handler, file_path);
    Ok(())
}

fn generate_controller_file(
    out_dir: &Path,
    handler: &str,
    response_fields: &[FieldDef],
    force: bool,
    example: Option<Value>,
) -> anyhow::Result<()> {
    let file_path = out_dir.join(format!("{}.rs", handler));
    if file_path.exists() && !force {
        println!("⚠️  Skipping existing controller file: {:?}", file_path);
        return Ok(());
    }

    let example_map = example
        .as_ref()
        .and_then(|v| match v {
            Value::Object(map) => Some(map.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let enriched_fields = response_fields
        .iter()
        .map(|field| {
            let value = example_map
                .get(&field.name)
                .map(|val| rust_literal_for_example(field, val))
                .unwrap_or_else(|| field.value.clone());
            FieldDef {
                name: field.name.clone(),
                ty: field.ty.clone(),
                optional: field.optional,
                value,
            }
        })
        .collect::<Vec<_>>();

    let context = ControllerTemplateData {
        handler_name: handler.to_string(),
        response_fields: enriched_fields,
        example: example
            .as_ref()
            .and_then(|v| serde_json::to_string_pretty(v).ok())
            .unwrap_or_default(),
        has_example: example.is_some(),
        example_json: "".to_string(), // TODO: unify or drop if unused
    };

    let rendered = context.render()?;
    fs::write(&file_path, rendered)?;
    println!("✅ Generated controller: {} → {:?}", handler, file_path);
    Ok(())
}

fn collect_component_schemas(spec: &oas3::OpenApiV3Spec) -> HashMap<String, TypeDefinition> {
    let mut schema_types = HashMap::new();
    if let Some(components) = spec.components.as_ref() {
        for (name, schema) in &components.schemas {
            match schema {
                oas3::spec::ObjectOrReference::Object(obj) => {
                    let schema_val = serde_json::to_value(obj).unwrap_or_default();
                    process_schema_type(name, &schema_val, &mut schema_types);
                }
                oas3::spec::ObjectOrReference::Ref { ref_path } => {
                    if let Some(resolved) = resolve_schema_ref(spec, ref_path) {
                        let schema_val = serde_json::to_value(resolved).unwrap_or_default();
                        process_schema_type(name, &schema_val, &mut schema_types);
                    }
                }
            }
        }
    }
    schema_types
}

fn to_camel_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn is_named_type(ty: &str) -> bool {
    let primitives = [
        "String",
        "i32",
        "i64",
        "f32",
        "f64",
        "bool",
        "Value",
        "serde_json::Value",
        "Vec<Value>",
    ];
    !primitives.contains(&ty)
        && !ty.starts_with("Vec<serde_json")
        && !ty.starts_with("Vec<Value>")
        && matches!(ty.chars().next(), Some('A'..='Z'))
}

fn rust_literal_for_example(field: &FieldDef, example: &Value) -> String {
    let literal = match example {
        Value::String(s) => format!("{:?}.to_string()", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => {
            let inner = items
                .iter()
                .map(|item| match item {
                    Value::String(s) => format!("{:?}.to_string().parse().unwrap()", s),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => "Default::default()".to_string(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{}]", inner)
        }
        _ => "Default::default()".to_string(),
    };

    if field.optional {
        format!("Some({})", literal)
    } else {
        literal
    }
}


fn value_to_rust_literal(value: &Value, wrap_in_option: bool) -> String {
    let raw = match value {
        Value::String(s) => format!("{:?}.to_string()", s),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(arr) => {
            let elems = arr
                .iter()
                .map(|v| match v {
                    Value::String(s) => format!("{:?}.to_string().parse().unwrap()", s),
                    _ => value_to_rust_literal(v, false),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{}]", elems)
        }
        Value::Object(_) | Value::Null => "Default::default()".to_string(),
    };

    if wrap_in_option {
        format!("Some({})", raw)
    } else {
        raw
    }
}


pub fn process_schema_type(
    name: &str,
    schema: &Value,
    schema_types: &mut HashMap<String, TypeDefinition>,
) {
    let type_name = to_camel_case(name);
    if !schema_types.contains_key(&type_name) {
        let type_fields = extract_fields(schema);
        if !type_fields.is_empty() {
            schema_types.insert(
                type_name.clone(),
                TypeDefinition {
                    name: type_name,
                    fields: type_fields,
                },
            );
        }
    }
}

pub fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];

    if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
        if schema_type == "array" {
            if let Some(items) = schema.get("items") {
                if let Some(ref_path) = items.get("$ref").and_then(|v| v.as_str()) {
                    if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                        fields.push(FieldDef {
                            name: "items".to_string(),
                            ty: format!("Vec<{}>", to_camel_case(name)),
                            optional: false,
                            value: "vec![]".to_string(),
                        });
                        return fields;
                    }
                }
                return extract_fields(items);
            }
        }
    }

    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(props) = schema.get("properties") {
        if let Some(map) = props.as_object() {
            for (name, prop) in map.iter() {
                let ty = if let Some(ref_path) = prop.get("$ref").and_then(|v| v.as_str()) {
                    if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                        to_camel_case(name)
                    } else {
                        "serde_json::Value".to_string()
                    }
                } else {
                    match prop.get("type").and_then(|t| t.as_str()) {
                        Some("string") => "String".to_string(),
                        Some("integer") => "i32".to_string(),
                        Some("number") => "f64".to_string(),
                        Some("boolean") => "bool".to_string(),
                        Some("array") => {
                            if let Some(items) = prop.get("items") {
                                if let Some(item_ref) = items.get("$ref").and_then(|v| v.as_str()) {
                                    if let Some(name) =
                                        item_ref.strip_prefix("#/components/schemas/")
                                    {
                                        format!("Vec<{}>", to_camel_case(name))
                                    } else {
                                        "Vec<serde_json::Value>".to_string()
                                    }
                                } else {
                                    "Vec<serde_json::Value>".to_string()
                                }
                            } else {
                                "Vec<serde_json::Value>".to_string()
                            }
                        }
                        Some("object") => "serde_json::Value".to_string(),
                        _ => "serde_json::Value".to_string(),
                    }
                };

                let optional = !required.iter().any(|r| r == name);
                let value = dummy_value::dummy_value(&ty)
                    .map(|v| if optional { format!("Some({})", v) } else { v })
                    .unwrap_or_else(|_| "Default::default()".to_string());

                fields.push(FieldDef {
                    name: name.clone(),
                    ty,
                    optional,
                    value,
                });
            }
        }
    }

    fields
}
