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

#[derive(Template)]
#[template(path = "handler_types.rs.txt")]
pub struct TypesTemplateData {
    pub types: BTreeMap<String, TypeDefinition>,
}

#[derive(Template)]
#[template(path = "handler.rs.txt")]
pub struct HandlerTemplateData {
    pub handler_name: String,
    pub request_fields: Vec<FieldDef>,
    pub response_fields: Vec<FieldDef>,
    pub response_is_array: bool,
    pub response_array_type: String,
    pub imports: Vec<String>,
    pub parameters: Vec<ParameterMeta>,
    pub sse: bool,
}

#[derive(Template)]
#[template(path = "controller.rs.txt")]
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
}

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
