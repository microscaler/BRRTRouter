use crate::dummy_value;
use crate::spec::{build_routes, load_spec, resolve_schema_ref, RouteMeta};
use askama::Template;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
#[template(path = "mod.rs.txt")]
pub struct ModRsTemplateData {
    pub modules: Vec<String>,
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

pub fn generate_project_from_spec(spec_path: &Path, force: bool) -> anyhow::Result<()> {
    let (routes, slug) = load_spec(spec_path.to_str().unwrap(), false)?;
    let base_dir = Path::new("examples").join(&slug);
    let src_dir = base_dir.join("src");
    let handler_dir = src_dir.join("handlers");
    let controller_dir = src_dir.join("controllers");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&handler_dir)?;
    fs::create_dir_all(&controller_dir)?;

    let mut schema_types = collect_component_schemas(spec_path)?;

    // Shared output state
    let mut seen = HashSet::new();
    let mut modules_handlers = Vec::new();
    let mut modules_controllers = Vec::new();
    let mut registry_entries = Vec::new();

    for route in routes.iter() {
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

        // Emit files
        let handler_path = handler_dir.join(format!("{}.rs", handler));
        let controller_path = controller_dir.join(format!("{}.rs", handler));
        write_handler(
            &handler_path,
            &handler,
            &request_fields,
            &response_fields,
            &imports,
            force,
        )?;
        write_controller(
            &controller_path,
            &handler,
            &response_fields,
            route.example.clone(),
            force,
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

    write_cargo_toml(&base_dir, &slug)?;
    write_main_rs(&src_dir, &slug, routes)?;
    write_types_rs(&handler_dir, &schema_types)?;
    write_registry_rs(&src_dir, &registry_entries)?;
    write_mod_rs(
        &handler_dir,
        &["types".to_string()]
            .into_iter()
            .chain(modules_handlers.clone())
            .collect::<Vec<_>>(),
        "handlers",
    )?;
    write_mod_rs(&controller_dir, &modules_controllers, "controllers")?;

    Ok(())
}

fn write_handler(
    path: &Path,
    handler: &str,
    req: &[FieldDef],
    res: &[FieldDef],
    imports: &BTreeSet<String>,
    force: bool,
) -> anyhow::Result<()> {
    if path.exists() && !force {
        println!("⚠️  Skipping existing handler file: {:?}", path);
        return Ok(());
    }
    let rendered = HandlerTemplateData {
        handler_name: handler.to_string(),
        request_fields: req.to_vec(),
        response_fields: res.to_vec(),
        imports: imports.iter().cloned().collect(),
    }
    .render()?;
    fs::write(path, rendered)?;
    println!("✅ Generated handler: {:?}", path);
    Ok(())
}

fn write_controller(
    path: &Path,
    handler: &str,
    res: &[FieldDef],
    example: Option<Value>,
    force: bool,
) -> anyhow::Result<()> {
    if path.exists() && !force {
        println!("⚠️  Skipping existing controller file: {:?}", path);
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
        example_json: "".to_string(),
    };

    fs::write(path, context.render()?)?;
    println!("✅ Generated controller: {:?}", path);
    Ok(())
}

fn write_mod_rs(dir: &Path, modules: &[String], label: &str) -> anyhow::Result<()> {
    let path = dir.join("mod.rs");
    let rendered = ModRsTemplateData {
        modules: modules.to_vec(),
    }
    .render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Updated mod.rs for {} → {:?}", label, path);
    Ok(())
}

fn write_registry_rs(dir: &Path, entries: &[RegistryEntry]) -> anyhow::Result<()> {
    let path = dir.join("registry.rs");
    let rendered = RegistryTemplateData {
        entries: entries.to_vec(),
    }
    .render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Generated registry.rs → {:?}", path);
    Ok(())
}

fn write_types_rs(dir: &Path, types: &HashMap<String, TypeDefinition>) -> anyhow::Result<()> {
    let path = dir.join("types.rs");
    let rendered = TypesTemplateData {
        types: types.clone(),
    }
    .render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Generated types.rs → {:?}", path);
    Ok(())
}

fn write_cargo_toml(base: &Path, slug: &str) -> anyhow::Result<()> {
    let rendered = CargoTomlTemplateData {
        name: slug.to_string(),
    }
    .render()?;
    fs::write(base.join("Cargo.toml"), rendered)?;
    println!("✅ Wrote Cargo.toml");
    Ok(())
}

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

fn collect_component_schemas(spec_path: &Path) -> anyhow::Result<HashMap<String, TypeDefinition>> {
    let spec: oas3::OpenApiV3Spec = if spec_path.extension().map(|s| s == "yaml").unwrap_or(false) {
        serde_yaml::from_str(&std::fs::read_to_string(spec_path)?)?
    } else {
        serde_json::from_str(&std::fs::read_to_string(spec_path)?)?
    };

    let mut types = HashMap::new();
    if let Some(components) = spec.components.as_ref() {
        for (name, schema) in &components.schemas {
            match schema {
                oas3::spec::ObjectOrReference::Object(obj) => {
                    let json = serde_json::to_value(obj).unwrap_or_default();
                    process_schema_type(name, &json, &mut types);
                }
                oas3::spec::ObjectOrReference::Ref { ref_path } => {
                    if let Some(resolved) = resolve_schema_ref(&spec, ref_path) {
                        let json = serde_json::to_value(resolved).unwrap_or_default();
                        process_schema_type(name, &json, &mut types);
                    }
                }
            }
        }
    }
    Ok(types)
}

fn to_camel_case(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
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

pub fn process_schema_type(
    name: &str,
    schema: &Value,
    types: &mut HashMap<String, TypeDefinition>,
) {
    let name = to_camel_case(name);
    if types.contains_key(&name) {
        return;
    }
    let fields = extract_fields(schema);
    if !fields.is_empty() {
        types.insert(name.clone(), TypeDefinition { name, fields });
    }
}

pub fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];
    if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
        if schema_type == "array" {
            if let Some(items) = schema.get("items") {
                let ty = if let Some(ref_path) = items.get("$ref").and_then(|v| v.as_str()) {
                    ref_path
                        .strip_prefix("#/components/schemas/")
                        .map(to_camel_case)
                        .unwrap_or_else(|| "serde_json::Value".to_string())
                } else {
                    "serde_json::Value".to_string()
                };

                fields.push(FieldDef {
                    name: "items".to_string(),
                    ty: format!("Vec<{}>", ty),
                    optional: false,
                    value: "vec![Default::default()]".to_string(),
                });
                return fields;
            }
        }
    }

    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            let ty = if let Some(r) = prop.get("$ref").and_then(|v| v.as_str()) {
                if let Some(name) = r.strip_prefix("#/components/schemas/") {
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
                                if let Some(name) = item_ref.strip_prefix("#/components/schemas/") {
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
                    _ => "serde_json::Value".to_string(),
                }
            };

            let optional = !required.contains(name);
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

    fields
}
