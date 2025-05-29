use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::spec::{load_spec};

use super::schema::{
    collect_component_schemas, extract_fields, parameter_to_field, process_schema_type,
    unique_handler_name, is_named_type, to_camel_case,
};
use super::templates::{
    write_cargo_toml, write_controller, write_handler, write_main_rs, write_mod_rs,
    write_registry_rs, write_types_rs, RegistryEntry,
};

pub fn generate_project_from_spec(spec_path: &Path, force: bool) -> anyhow::Result<PathBuf> {
    let (mut routes, slug) = load_spec(spec_path.to_str().unwrap())?;
    let base_dir = Path::new("examples").join(&slug);
    let src_dir = base_dir.join("src");
    let handler_dir = src_dir.join("handlers");
    let controller_dir = src_dir.join("controllers");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&handler_dir)?;
    fs::create_dir_all(&controller_dir)?;

    let spec_copy_path = base_dir.join("openapi.yaml");
    if !spec_copy_path.exists() || force {
        fs::copy(spec_path, &spec_copy_path)?;
        println!("✅ Copied spec to {:?}", spec_copy_path);
    }

    let mut schema_types = collect_component_schemas(spec_path)?;

    let mut seen = HashSet::new();
    let mut modules_handlers = Vec::new();
    let mut modules_controllers = Vec::new();
    let mut registry_entries = Vec::new();

    for route in routes.iter_mut() {
        let handler = unique_handler_name(&mut seen, &route.handler_name);
        route.handler_name = handler.clone();

        let mut request_fields = route.request_schema.as_ref().map_or(vec![], extract_fields);
        for param in &route.parameters {
            request_fields.push(parameter_to_field(param));
        }
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

        let handler_path = handler_dir.join(format!("{}.rs", handler));
        let controller_path = controller_dir.join(format!("{}.rs", handler));
        write_handler(
            &handler_path,
            &handler,
            &request_fields,
            &response_fields,
            &imports,
            &route.parameters,
            force,
        )?;
        let controller_struct = format!("{}Controller", to_camel_case(&handler));
        write_controller(
            &controller_path,
            &handler,
            &controller_struct,
            &response_fields,
            route.example.clone(),
            force,
        )?;

        modules_handlers.push(handler.clone());
        modules_controllers.push(handler.clone());
        registry_entries.push(RegistryEntry {
            name: handler.clone(),
            request_type: format!("{}::Request", handler),
            controller_struct: controller_struct.clone(),
            parameters: route.parameters.clone(),
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

    Ok(base_dir)
}

pub fn format_project(dir: &Path) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("fmt").current_dir(dir);
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("cargo fmt failed");
    }
    Ok(())
}

