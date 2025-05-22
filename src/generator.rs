// generator.rs
use crate::spec::load_spec;
use crate::dummy_value;
use askama::Template;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Template)]
#[template(path = "handler.rs.txt")]
pub struct HandlerTemplateData {
    pub handler_name: String,
    pub request_fields: Vec<FieldDef>,
    pub response_fields: Vec<FieldDef>,
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

    let routes = load_spec(spec_path.to_str().unwrap(), false)?;
    fs::create_dir_all(out_dir)?;
    let controller_dir = Path::new("src/controllers");
    fs::create_dir_all(controller_dir)?;

    let mut seen = HashSet::new();
    let mut mod_lines_handlers = Vec::new();
    let mut mod_lines_controllers = Vec::new();
    let mut registry_entries = Vec::new();

    for route in routes {
        let handler = route.handler_name.clone();
        if !seen.insert(handler.clone()) {
            continue;
        }

        println!("🔍 Route: {}", route.handler_name);
        println!("📦 Request schema: {:#?}", route.request_schema);
        println!("📦 Response schema: {:#?}", route.response_schema);

        let request_fields = route.request_schema.as_ref().map_or(vec![], extract_fields);
        let response_fields = route.response_schema.as_ref().map_or(vec![], extract_fields);

        println!("🛠 Generating for handler: {}", handler);
        println!("📨 Request Fields: {:#?}", request_fields);
        println!("📤 Response Fields: {:#?}", response_fields);

        let file_path = out_dir.join(format!("{}.rs", handler));
        if !file_path.exists() || force {
            let context = HandlerTemplateData {
                handler_name: handler.clone(),
                request_fields: request_fields.clone(),
                response_fields: response_fields.clone(),
            };
            let rendered = context.render()?;
            fs::write(&file_path, rendered)?;
            println!("✅ Generated handler: {} → {:?}", handler, file_path);
        } else {
            println!("⚠️  Skipping existing handler file: {:?}", file_path);
        }

        let controller_path = controller_dir.join(format!("{}.rs", handler));
        if !controller_path.exists() || force {
            let context = ControllerTemplateData {
                handler_name: handler.clone(),
                response_fields: response_fields.clone(),
            };
            let rendered = context.render()?;
            fs::write(&controller_path, rendered)?;
            println!("✅ Generated controller: {} → {:?}", handler, controller_path);
        } else {
            println!("⚠️  Skipping existing controller file: {:?}", controller_path);
        }

        mod_lines_handlers.push(format!("pub mod {};", handler));
        mod_lines_controllers.push(format!("pub mod {};", handler));
        registry_entries.push(RegistryEntry { name: handler });
    }

    let mod_rs_handlers = out_dir.join("mod.rs");
    let mut file = fs::File::create(&mod_rs_handlers)?;
    for line in &mod_lines_handlers {
        writeln!(file, "{}", line)?;
    }
    println!("✅ Updated mod.rs for handlers → {:?}", mod_rs_handlers);

    let mod_rs_controllers = controller_dir.join("mod.rs");
    let mut file = fs::File::create(&mod_rs_controllers)?;
    for line in &mod_lines_controllers {
        writeln!(file, "{}", line)?;
    }
    println!("✅ Updated mod.rs for controllers → {:?}", mod_rs_controllers);

    let registry_path = Path::new("src/registry.rs");
    let context = RegistryTemplateData { entries: registry_entries };
    let rendered = context.render()?;
    fs::write(&registry_path, rendered)?;
    println!("✅ Generated registry.rs → {:?}", registry_path);

    Ok(())
}

fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];

    if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
        if schema_type == "array" {
            if let Some(items) = schema.get("items") {
                return extract_fields(items);
            }
        }
    }

    let required = schema.get("required").and_then(|v| v.as_array()).map(|arr| {
        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>()
    }).unwrap_or_default();

    if let Some(props) = schema.get("properties") {
        if let Some(map) = props.as_object() {
            for (name, prop) in map.iter() {
                let ty = match prop.get("type").and_then(|t| t.as_str()) {
                    Some("string") => "String",
                    Some("integer") => "i32",
                    Some("number") => "f64",
                    Some("boolean") => "bool",
                    Some("array") => "Vec<Value>",
                    Some("object") => "serde_json::Value",
                    _ => "serde_json::Value",
                }.to_string();

                let optional = !required.contains(name);
                let value = dummy_value::dummy_value(&ty).unwrap_or_else(|_| "Default::default()".to_string());

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
