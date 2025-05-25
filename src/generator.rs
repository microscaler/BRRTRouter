use crate::dummy_value;
use crate::spec::{build_routes, resolve_schema_ref};
use askama::Template;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

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

    let mut schema_types: HashMap<String, TypeDefinition> = HashMap::new();
    if let Some(components) = spec.components.as_ref() {
        for (name, schema) in &components.schemas {
            match schema {
                oas3::spec::ObjectOrReference::Object(obj) => {
                    let schema_val = serde_json::to_value(obj)?;
                    process_schema_type(name, &schema_val, &mut schema_types);
                }
                oas3::spec::ObjectOrReference::Ref { ref_path } => {
                    if let Some(resolved) = resolve_schema_ref(&spec, ref_path) {
                        let schema_val = serde_json::to_value(resolved)?;
                        process_schema_type(name, &schema_val, &mut schema_types);
                    }
                }
            }
        }
    }

    fs::create_dir_all(out_dir)?;
    let controller_dir = Path::new("src/controllers");
    fs::create_dir_all(controller_dir)?;

    let mut seen = HashSet::new();
    let mut mod_lines_handlers = Vec::new();
    let mut mod_lines_controllers = Vec::new();
    let mut registry_entries = Vec::new();

    for route in routes {
        let handler = &route.handler_name;
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
                .and_then(|s| s.strip_suffix('>'))
                .unwrap_or(&field.ty);

            if is_named_type(inner, &schema_types) {
                imports.insert(inner.to_string());
            }
        }

        let file_path = out_dir.join(format!("{}.rs", handler));
        if !file_path.exists() || force {
            let context = HandlerTemplateData {
                handler_name: handler.clone(),
                request_fields: request_fields.clone(),
                response_fields: response_fields.clone(),
                imports: imports.into_iter().collect(),
            };
            fs::write(&file_path, context.render()?)?;
        }

        let controller_path = controller_dir.join(format!("{}.rs", handler));
        if !controller_path.exists() || force {
            let context = ControllerTemplateData {
                handler_name: handler.clone(),
                response_fields: response_fields.clone(),
            };
            fs::write(&controller_path, context.render()?)?;
        }

        mod_lines_handlers.push(format!("pub mod {};", handler));
        mod_lines_controllers.push(format!("pub mod {};", handler));
        registry_entries.push(RegistryEntry {
            name: handler.clone(),
        });
    }

    fs::write(out_dir.join("mod.rs"), mod_lines_handlers.join("\n"))?;
    fs::write("src/controllers/mod.rs", mod_lines_controllers.join("\n"))?;

    fs::write(
        "src/registry.rs",
        RegistryTemplateData {
            entries: registry_entries,
        }
        .render()?,
    )?;

    fs::write(
        out_dir.join("types.rs"),
        TypesTemplateData {
            types: schema_types,
        }
        .render()?,
    )?;

    Ok(())
}

fn to_camel_case(s: &str) -> String {
    s.split('_')
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn is_named_type(ty: &str, known_types: &HashMap<String, TypeDefinition>) -> bool {
    let primitives = [
        "String",
        "i32",
        "i64",
        "f32",
        "f64",
        "bool",
        "serde_json::Value",
        "Vec<Value>",
    ];
    !primitives.contains(&ty) && known_types.contains_key(ty)
}

pub fn process_schema_type(
    name: &str,
    schema: &Value,
    schema_types: &mut HashMap<String, TypeDefinition>,
) {
    let type_name = to_camel_case(name);
    if !schema_types.contains_key(&type_name) {
        let fields = extract_fields(schema);
        if !fields.is_empty() {
            schema_types.insert(
                type_name.clone(),
                TypeDefinition {
                    name: type_name,
                    fields,
                },
            );
        }
    }
}

pub fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];

    if let Some(schema_type) = schema.get("type").and_then(Value::as_str) {
        if schema_type == "array" {
            if let Some(items) = schema.get("items") {
                if let Some(ref_path) = items.get("$ref").and_then(Value::as_str) {
                    if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                        return vec![FieldDef {
                            name: "items".to_string(),
                            ty: format!("Vec<{}>", to_camel_case(name)),
                            optional: false,
                            value: "vec![]".to_string(),
                        }];
                    }
                }
                return extract_fields(items);
            }
        }
    }

    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (name, prop) in props {
            let ty = if let Some(ref_path) = prop.get("$ref").and_then(Value::as_str) {
                ref_path
                    .strip_prefix("#/components/schemas/")
                    .map(to_camel_case)
                    .unwrap_or_else(|| "serde_json::Value".to_string())
            } else {
                match prop.get("type").and_then(Value::as_str) {
                    Some("string") => "String".to_string(),
                    Some("integer") => "i32".to_string(),
                    Some("number") => "f64".to_string(),
                    Some("boolean") => "bool".to_string(),
                    Some("array") => {
                        if let Some(items) = prop.get("items") {
                            if let Some(inner_ref) = items.get("$ref").and_then(Value::as_str) {
                                if let Some(name) = inner_ref.strip_prefix("#/components/schemas/")
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

            let optional = !required.contains(&name.as_str());
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
