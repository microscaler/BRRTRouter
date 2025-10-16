use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::spec::load_spec;

use crate::generator::schema::{
    collect_component_schemas, extract_fields, is_named_type, parameter_to_field,
    process_schema_type, to_camel_case, unique_handler_name,
};
use crate::generator::templates::{
    write_cargo_toml, write_controller, write_handler, write_main_rs, write_mod_rs,
    write_openapi_index, write_registry_rs, write_static_index, write_types_rs, RegistryEntry,
};

use anyhow::Context;

/// Configuration for selective code generation
///
/// Controls which parts of the project are regenerated. Useful for incremental
/// updates where only specific files need to be modified.
#[derive(Debug, Clone, Copy, Default)]
pub struct GenerationScope {
    /// Generate handler modules (request/response types and handler skeletons)
    pub handlers: bool,
    /// Generate controller modules (coroutine dispatchers)
    pub controllers: bool,
    /// Generate type definitions from OpenAPI schemas
    pub types: bool,
    /// Generate registry module (handler registration)
    pub registry: bool,
    /// Generate main.rs entry point
    pub main: bool,
    /// Generate documentation files (OpenAPI spec, HTML docs)
    pub docs: bool,
}

impl GenerationScope {
    /// Create a scope that enables all generation options
    pub fn all() -> Self {
        Self {
            handlers: true,
            controllers: true,
            types: true,
            registry: true,
            main: true,
            docs: true,
        }
    }
}

/// Generate a complete Rust project from an OpenAPI specification
///
/// Creates a new project with handlers, controllers, types, and all supporting files
/// in the `examples/` directory. This is the simple interface that generates everything.
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI specification file
/// * `force` - Overwrite existing files without prompting
///
/// # Returns
///
/// The path to the generated project directory
///
/// # Errors
///
/// Returns an error if spec loading, code generation, or file I/O fails.
pub fn generate_project_from_spec(spec_path: &Path, force: bool) -> anyhow::Result<PathBuf> {
    generate_project_with_options(spec_path, force, false, &GenerationScope::all())
}

/// Generate a Rust project with fine-grained control over what gets generated
///
/// Allows selective regeneration of specific parts (handlers, controllers, etc.)
/// and supports dry-run mode for previewing changes.
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI specification file
/// * `force` - Overwrite existing files without prompting
/// * `dry_run` - Show what would be generated without writing files
/// * `scope` - Which parts of the project to generate
///
/// # Returns
///
/// The path to the generated project directory
///
/// # Errors
///
/// Returns an error if spec loading, code generation, or file I/O fails.
pub fn generate_project_with_options(
    spec_path: &Path,
    force: bool,
    dry_run: bool,
    scope: &GenerationScope,
) -> anyhow::Result<PathBuf> {
    let mut created: Vec<String> = Vec::new();
    let mut updated: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let (mut routes, slug) = load_spec(spec_path.to_str().unwrap())?;
    let base_dir = Path::new("examples").join(&slug);
    let src_dir = base_dir.join("src");
    let handler_dir = src_dir.join("handlers");
    let controller_dir = src_dir.join("controllers");
    let doc_dir = base_dir.join("doc");
    let static_dir = base_dir.join("static_site");
    let config_dir = base_dir.join("config");
    if !dry_run {
        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&handler_dir)?;
        fs::create_dir_all(&controller_dir)?;
        fs::create_dir_all(&doc_dir)?;
        fs::create_dir_all(&static_dir)?;
        fs::create_dir_all(&config_dir)?;
    }

    let spec_copy_path = doc_dir.join("openapi.yaml");
    // Spec Copy Safety: canonicalize and avoid self-copy/truncation; clear logs; honor --force
    let source_canon = fs::canonicalize(spec_path)
        .with_context(|| format!("Failed to canonicalize spec source: {spec_path:?}"))?;
    let doc_dir_canon = fs::canonicalize(&doc_dir)
        .with_context(|| format!("Failed to canonicalize doc dir: {doc_dir:?}"))?;
    let dest_path = doc_dir_canon.join("openapi.yaml");

    if source_canon == dest_path {
        println!("⚠️  Skipping spec copy: source and destination are the same → {dest_path:?}",);
        skipped.push(format!("spec: same-path → {dest_path:?}"));
    } else if !spec_copy_path.exists() || force {
        println!("📄 Copying spec from {source_canon:?} → {spec_copy_path:?}",);
        if dry_run {
            println!("🔎 Dry-run: would copy spec (skipped)");
            if spec_copy_path.exists() {
                updated.push(format!("spec: {spec_copy_path:?}"));
            } else {
                created.push(format!("spec: {spec_copy_path:?}"));
            }
        } else {
            fs::copy(&source_canon, &spec_copy_path).with_context(|| {
                format!("Failed to copy spec from {source_canon:?} to {spec_copy_path:?}")
            })?;
            println!("✅ Copied spec to {spec_copy_path:?}");
            if spec_copy_path.exists() {
                // Post-copy, treat as created if it didn't exist before; approximate using force flag
                if force {
                    updated.push(format!("spec: {spec_copy_path:?}"));
                } else {
                    created.push(format!("spec: {spec_copy_path:?}"));
                }
            }
        }
    } else {
        println!("ℹ️  Spec already present at {spec_copy_path:?} (use --force to overwrite)",);
        skipped.push(format!("spec: exists → {spec_copy_path:?}"));
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

        let handler_path = handler_dir.join(format!("{handler}.rs"));
        let controller_path = controller_dir.join(format!("{handler}.rs"));
        if scope.handlers {
            let existed = handler_path.exists();
            if dry_run {
                if existed && !force {
                    skipped.push(format!("handler: skip existing → {handler_path:?}"));
                } else if existed && force {
                    updated.push(format!("handler: {handler_path:?}"));
                } else {
                    created.push(format!("handler: {handler_path:?}"));
                }
            } else {
                write_handler(
                    &handler_path,
                    &handler,
                    &request_fields,
                    &response_fields,
                    &imports,
                    &route.parameters,
                    route.sse,
                    force,
                )?;
                if existed && force {
                    updated.push(format!("handler: {handler_path:?}"));
                } else if !existed {
                    created.push(format!("handler: {handler_path:?}"));
                } else {
                    skipped.push(format!("handler: skip existing → {handler_path:?}"));
                }
            }
        } else {
            println!("🔎 Dry-run/only: skipping handler generation for {handler}");
            skipped.push(format!("handler: only/skip → {handler_path:?}"));
        }
        let controller_struct = format!("{}Controller", to_camel_case(&handler));
        if scope.controllers {
            let existed = controller_path.exists();
            if dry_run {
                if existed && !force {
                    skipped.push(format!("controller: skip existing → {controller_path:?}"));
                } else if existed && force {
                    updated.push(format!("controller: {controller_path:?}"));
                } else {
                    created.push(format!("controller: {controller_path:?}"));
                }
            } else {
                write_controller(
                    &controller_path,
                    &handler,
                    &controller_struct,
                    &response_fields,
                    route.example.clone(),
                    route.sse,
                    force,
                )?;
                if existed && force {
                    updated.push(format!("controller: {controller_path:?}"));
                } else if !existed {
                    created.push(format!("controller: {controller_path:?}"));
                } else {
                    skipped.push(format!("controller: skip existing → {controller_path:?}"));
                }
            }
        } else {
            println!("🔎 Dry-run/only: skipping controller generation for {handler}");
            skipped.push(format!("controller: only/skip → {controller_path:?}"));
        }

        modules_handlers.push(handler.clone());
        modules_controllers.push(handler.clone());
        registry_entries.push(RegistryEntry {
            name: handler.clone(),
            request_type: format!("{handler}::Request"),
            controller_struct: controller_struct.clone(),
            parameters: route.parameters.clone(),
        });

        if let Some(schema) = &route.request_schema {
            let name = format!("{handler}Request");
            process_schema_type(&name, schema, &mut schema_types);
        }
        if let Some(schema) = &route.response_schema {
            let name = format!("{handler}Response");
            process_schema_type(&name, schema, &mut schema_types);
        }
    }

    if scope.main {
        let cargo_path = base_dir.join("Cargo.toml");
        let main_path = src_dir.join("main.rs");
        let cargo_existed = cargo_path.exists();
        let main_existed = main_path.exists();
        if dry_run {
            if cargo_existed && !force {
                skipped.push(format!("cargo: skip existing → {cargo_path:?}"));
            } else if cargo_existed && force {
                updated.push(format!("cargo: {cargo_path:?}"));
            } else {
                created.push(format!("cargo: {cargo_path:?}"));
            }
            if main_existed && !force {
                skipped.push(format!("main: skip existing → {main_path:?}"));
            } else if main_existed && force {
                updated.push(format!("main: {main_path:?}"));
            } else {
                created.push(format!("main: {main_path:?}"));
            }
        } else {
            write_cargo_toml(&base_dir, &slug)?;
            write_main_rs(&src_dir, &slug, routes.clone())?;
            if cargo_existed && force {
                updated.push(format!("cargo: {cargo_path:?}"));
            } else if !cargo_existed {
                created.push(format!("cargo: {cargo_path:?}"));
            } else {
                skipped.push(format!("cargo: skip existing → {cargo_path:?}"));
            }
            if main_existed && force {
                updated.push(format!("main: {main_path:?}"));
            } else if !main_existed {
                created.push(format!("main: {main_path:?}"));
            } else {
                skipped.push(format!("main: skip existing → {main_path:?}"));
            }
        }
    } else {
        println!("🔎 Dry-run/only: skipping Cargo.toml/main.rs generation");
    }
    if scope.docs {
        let docs_path = doc_dir.join("index.html");
        let static_path = static_dir.join("index.html");
        let docs_existed = docs_path.exists();
        let static_existed = static_path.exists();
        if dry_run {
            if docs_existed && !force {
                skipped.push(format!("docs: skip existing → {docs_path:?}"));
            } else if docs_existed && force {
                updated.push(format!("docs: {docs_path:?}"));
            } else {
                created.push(format!("docs: {docs_path:?}"));
            }
            if static_existed && !force {
                skipped.push(format!("static: skip existing → {static_path:?}"));
            } else if static_existed && force {
                updated.push(format!("static: {static_path:?}"));
            } else {
                created.push(format!("static: {static_path:?}"));
            }
        } else {
            write_openapi_index(&doc_dir)?;
            write_static_index(&static_dir)?;
            super::super::templates::write_default_config(&config_dir)?;
            if docs_existed && force {
                updated.push(format!("docs: {docs_path:?}"));
            } else if !docs_existed {
                created.push(format!("docs: {docs_path:?}"));
            } else {
                skipped.push(format!("docs: skip existing → {docs_path:?}"));
            }
            if static_existed && force {
                updated.push(format!("static: {static_path:?}"));
            } else if !static_existed {
                created.push(format!("static: {static_path:?}"));
            } else {
                skipped.push(format!("static: skip existing → {static_path:?}"));
            }
        }
    } else {
        println!("🔎 Dry-run/only: skipping docs/static generation");
    }
    if scope.types {
        let types_path = handler_dir.join("types.rs");
        let types_existed = types_path.exists();
        if dry_run {
            if types_existed && !force {
                skipped.push(format!("types: skip existing → {types_path:?}"));
            } else if types_existed && force {
                updated.push(format!("types: {types_path:?}"));
            } else {
                created.push(format!("types: {types_path:?}"));
            }
        } else {
            write_types_rs(&handler_dir, &schema_types)?;
            if types_existed && force {
                updated.push(format!("types: {types_path:?}"));
            } else if !types_existed {
                created.push(format!("types: {types_path:?}"));
            } else {
                skipped.push(format!("types: skip existing → {types_path:?}"));
            }
        }
    } else {
        println!("🔎 Dry-run/only: skipping types.rs generation");
    }
    if scope.registry {
        let registry_path = src_dir.join("registry.rs");
        let registry_existed = registry_path.exists();
        if dry_run {
            if registry_existed && !force {
                skipped.push(format!("registry: skip existing → {registry_path:?}"));
            } else if registry_existed && force {
                updated.push(format!("registry: {registry_path:?}"));
            } else {
                created.push(format!("registry: {registry_path:?}"));
            }
        } else {
            write_registry_rs(&src_dir, &registry_entries)?;
            if registry_existed && force {
                updated.push(format!("registry: {registry_path:?}"));
            } else if !registry_existed {
                created.push(format!("registry: {registry_path:?}"));
            } else {
                skipped.push(format!("registry: skip existing → {registry_path:?}"));
            }
        }
    } else {
        println!("🔎 Dry-run/only: skipping registry.rs generation");
    }
    write_mod_rs(
        &handler_dir,
        &["types".to_string()]
            .into_iter()
            .chain(modules_handlers.clone())
            .collect::<Vec<_>>(),
        "handlers",
    )?;
    write_mod_rs(&controller_dir, &modules_controllers, "controllers")?;

    // Human-readable summary
    println!("\n──────────────── Generation Summary ────────────────");
    if !created.is_empty() {
        println!("🆕 Created ({}):", created.len());
        for c in &created {
            println!("  • {c}");
        }
    }
    if !updated.is_empty() {
        println!("♻️  Updated ({}):", updated.len());
        for u in &updated {
            println!("  • {u}");
        }
    }
    if !skipped.is_empty() {
        println!("⏭️  Skipped ({}):", skipped.len());
        for s in &skipped {
            println!("  • {s}");
        }
    }
    println!("──────────────────────────────────────────────────\n");
    Ok(base_dir)
}
