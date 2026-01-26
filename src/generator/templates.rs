use askama::Template;
// Remove explicit filters import; not needed and causes unresolved symbol errors
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

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
    /// Computed stack size for the coroutine in bytes
    pub stack_size_bytes: usize,
}

/// Parameters for writing implementation controller stub files
///
/// Groups related parameters to avoid functions with too many arguments.
pub struct ImplControllerStubParams<'a> {
    /// Path where the stub file should be written
    pub path: &'a Path,
    /// Handler name
    pub handler: &'a str,
    /// Struct name for the controller
    pub struct_name: &'a str,
    /// Crate name for imports
    pub crate_name: &'a str,
    /// Request field definitions
    pub req_fields: &'a [FieldDef],
    /// Response field definitions
    pub res_fields: &'a [FieldDef],
    /// Import statements needed
    pub imports: &'a BTreeSet<String>,
    /// Whether this is a server-sent events endpoint
    pub sse: bool,
    /// Optional example data for the response
    pub example: Option<Value>,
    /// Whether to overwrite existing files
    pub force: bool,
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
    /// Version for [package].version
    pub version: String,
    /// Whether to use workspace dependencies (true) or direct path dependencies (false)
    pub use_workspace_deps: bool,
    /// Relative path to BRRTRouter (only used when use_workspace_deps is false)
    pub brrtrouter_path: String,
    /// Relative path to brrtrouter_macros (only used when use_workspace_deps is false)
    pub brrtrouter_macros_path: String,
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
    /// Whether to use `crate::registry` (true) or `{{ name }}::registry` (false)
    /// Defaults to false for backward compatibility with petstore example
    pub use_crate_prefix: bool,
}

/// Template for generating OpenAPI documentation HTML
#[derive(Template)]
#[template(path = "openapi.index.html", escape = "none")]
pub struct OpenapiIndexTemplate;

/// Template for generating static site index.html
#[derive(Template)]
#[template(path = "static.index.html", escape = "none")]
pub struct StaticIndexTemplate;

/// Template for generating lib.rs (library entry point)
#[derive(Template)]
#[template(path = "lib.rs.txt", escape = "none")]
pub struct LibRsTemplate;

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

    // COMPLEX LOGIC: Extract example data from OpenAPI response example
    // The example can be an object (most common) or an array (for list endpoints)
    // We convert it to a map so we can look up values by field name
    let example_map = example
        .as_ref()
        .and_then(|v| match v {
            Value::Object(map) => Some(map.clone()),
            _ => None, // Not an object, we'll handle arrays separately
        })
        .unwrap_or_default();

    // ENRICHMENT: Replace each field's dummy value with actual example data if available
    // This ensures generated controllers return realistic example responses from the OpenAPI spec
    let enriched_fields = res
        .iter()
        .map(|field| {
            // Try to find this field in the example data
            // Use original_name for lookup (JSON key) but name for code generation (Rust identifier)
            let value = example_map
                .get(&field.original_name) // Look up by original JSON name (e.g., "type" not "r#type")
                .map(|val| rust_literal_for_example(field, val)) // Convert JSON → Rust literal
                .unwrap_or_else(|| field.value.clone()); // Fallback to dummy value

            // Clone field with enriched value
            FieldDef {
                name: field.name.clone(),
                original_name: field.original_name.clone(),
                ty: field.ty.clone(),
                optional: field.optional,
                value, // Use enriched value with actual example data
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
    // COMPLEX: Detect if response is an array (list endpoints like GET /pets)
    // Array responses have a single field named "items" with type Vec<T>
    let response_is_array = res.len() == 1 && res[0].name == "items";

    // TRICKY ARRAY HANDLING: Generate vec![] literal for array responses
    // We have three possible sources of data, prioritized:
    // 1. OpenAPI example that is itself an array → use it directly
    // 2. OpenAPI example that is an object → extract the "items" field
    // 3. No example → use dummy data
    let array_literal = if response_is_array {
        // Check if we have an OpenAPI example to work with
        if let Some(ref ex) = example {
            if ex.is_array() {
                // BEST CASE: Example is already an array like [{"id": 1}, {"id": 2}]
                // Create a temporary FieldDef to convert the whole array
                let items_field = FieldDef {
                    name: "items".to_string(),
                    original_name: "items".to_string(),
                    ty: res[0].ty.clone(), // Vec<T> where T is the element type
                    optional: false,
                    value: String::new(), // Not used for this purpose
                };
                // Convert entire JSON array to Rust vec![] literal
                super::schema::rust_literal_for_example(&items_field, ex)
            } else {
                // Example is an object but response is array - use enriched field
                // (This can happen with inconsistent OpenAPI specs)
                enriched_fields
                    .first()
                    .map(|f| f.value.clone())
                    .unwrap_or_else(|| "vec![]".to_string())
            }
        } else {
            // NO EXAMPLE: Fall back to enriched field's dummy value
            enriched_fields
                .first()
                .map(|f| f.value.clone())
                .unwrap_or_else(|| "vec![]".to_string())
        }
    } else {
        // Not an array response, no array literal needed
        String::new()
    };
    let context = ControllerTemplateData {
        handler_name: handler.to_string(),
        struct_name: struct_name.to_string(),
        response_fields: enriched_fields, // Last usage - move instead of clone
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

/// Write a mod.rs file with module declarations (internal helper)
///
/// Generates a `mod.rs` file that declares all submodules in a directory.
/// Used internally by the generator to create handler/controller module files.
///
/// # Arguments
///
/// * `dir` - Directory where mod.rs will be created
/// * `modules` - List of module names to declare
/// * `label` - Label for success message (e.g., "handlers", "controllers")
///
/// # Errors
///
/// Returns an error if file writing fails
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

/// Write the lib.rs file
///
/// Generates the library entry point that re-exports the registry module.
/// This allows the generated crate to be used as a library dependency in tests.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `src/`)
///
/// # Errors
///
/// Returns an error if template rendering or file writing fails
pub fn write_lib_rs(dir: &Path) -> anyhow::Result<()> {
    let path = dir.join("lib.rs");
    let rendered = LibRsTemplate.render()?;
    fs::write(path.clone(), rendered)?;
    println!("✅ Generated lib.rs → {path:?}");
    Ok(())
}

/// Write the types.rs file with type definitions (internal helper)
///
/// Generates a `types.rs` file containing all Rust struct definitions extracted
/// from OpenAPI component schemas.
///
/// # Arguments
///
/// * `dir` - Output directory (typically `src/handlers/`)
/// * `types` - Map of type names to their definitions
///
/// # Errors
///
/// Returns an error if template rendering or file writing fails
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

/// Write the Cargo.toml file for the generated project (internal helper)
///
/// Generates the Cargo.toml manifest with project name and dependencies.
///
/// # Arguments
///
/// * `base` - Project root directory
/// * `slug` - Project name slug (URL-safe identifier)
///
/// # Errors
///
/// Returns an error if template rendering or file writing fails
pub(crate) fn write_cargo_toml(base: &Path, slug: &str) -> anyhow::Result<()> {
    // Detect if we're in a workspace and if workspace has brrtrouter dependencies
    let use_workspace_deps = detect_workspace_with_brrtrouter_deps(base);
    write_cargo_toml_with_options(base, slug, use_workspace_deps, None, None)
}

/// Detect if output directory is in a workspace that has brrtrouter in workspace.dependencies
///
/// This function handles two scenarios:
/// 1. Workspaces WITH workspace.dependencies (e.g., RERP microservices workspace)
///    - Has [workspace], workspace.dependencies, and brrtrouter in workspace.dependencies
///    - Returns true → generated Cargo.toml uses `brrtrouter = { workspace = true }`
/// 2. Workspaces WITHOUT workspace.dependencies (e.g., BRRTRouter workspace)
///    - Has [workspace] but no workspace.dependencies section
///    - Returns false → generated Cargo.toml uses `brrtrouter = { path = "../.." }`
///
/// The detection is precise: it requires ALL of:
/// - [workspace] section exists
/// - [workspace.dependencies] section exists
/// - "brrtrouter" appears in the workspace.dependencies section
pub(crate) fn detect_workspace_with_brrtrouter_deps(output_dir: &Path) -> bool {
    let mut current = output_dir;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                // Must have [workspace] section
                if !contents.contains("[workspace]") {
                    match current.parent() {
                        Some(parent) => {
                            current = parent;
                            continue;
                        }
                        None => break,
                    }
                }
                
                // Must have [workspace.dependencies] section (not just [workspace])
                if !contents.contains("[workspace.dependencies]") {
                    match current.parent() {
                        Some(parent) => {
                            current = parent;
                            continue;
                        }
                        None => break,
                    }
                }
                
                // Check if brrtrouter is defined in workspace.dependencies
                // Look for pattern: brrtrouter = { ... } within [workspace.dependencies]
                let lines: Vec<&str> = contents.lines().collect();
                let mut in_workspace_deps = false;
                for line in lines {
                    let trimmed = line.trim();
                    if trimmed == "[workspace.dependencies]" {
                        in_workspace_deps = true;
                        continue;
                    }
                    if trimmed.starts_with('[') && in_workspace_deps {
                        // Left [workspace.dependencies] section
                        break;
                    }
                    if in_workspace_deps {
                        // Check if this line defines brrtrouter
                        if trimmed.starts_with("brrtrouter") && trimmed.contains('=') {
                            return true;
                        }
                    }
                }
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    false
}

/// Write Cargo.toml with options
///
/// # Arguments
///
/// * `base` - Project root directory
/// * `slug` - Project name slug
/// * `use_workspace_deps` - If true, use workspace dependencies; if false, calculate relative paths
/// * `brrtrouter_root` - Optional path to BRRTRouter root (for calculating relative paths)
/// * `version` - Optional version string (defaults to "0.1.0" if None)
pub(crate) fn write_cargo_toml_with_options(
    base: &Path,
    slug: &str,
    use_workspace_deps: bool,
    brrtrouter_root: Option<&Path>,
    version: Option<String>,
) -> anyhow::Result<()> {
    eprintln!(
        "DEBUG: use_workspace_deps={}, base={:?}, brrtrouter_root={:?}",
        use_workspace_deps, base, brrtrouter_root
    );
    let (brrtrouter_path, brrtrouter_macros_path) = if use_workspace_deps {
        (String::new(), String::new())
    } else {
        // Calculate relative path from output directory to BRRTRouter
        let brrtrouter_base = brrtrouter_root
            .map(|p| p.to_path_buf())
            .or_else(|| {
                // Try to find BRRTRouter by looking for its Cargo.toml in parent directories
                // This handles the case where we're generating from within BRRTRouter
                let mut current = base;
                loop {
                    let cargo_toml = current.join("Cargo.toml");
                    if cargo_toml.exists() {
                        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                            if content.contains("name = \"brrtrouter\"") {
                                return Some(current.to_path_buf());
                            }
                        }
                    }
                    match current.parent() {
                        Some(parent) => current = parent,
                        None => break,
                    }
                }
                None
            })
            .unwrap_or_else(|| {
                // Default fallback: assume BRRTRouter is at ../.. (petstore case)
                base.parent()
                    .and_then(|p| p.parent())
                    .unwrap_or(base)
                    .to_path_buf()
            });

        // Calculate relative path from base to brrtrouter_base
        // Ensure both paths are absolute for reliable calculation
        let brrtrouter_base_clone = brrtrouter_base.clone(); // Clone for debug output
        let base_abs = if base.is_absolute() {
            base.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(base)
        };
        let brrtrouter_abs = if brrtrouter_base.is_absolute() {
            brrtrouter_base // Already a PathBuf, no need to clone
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(brrtrouter_base)
        };

        let base_canon = base_abs.canonicalize().unwrap_or(base_abs);
        let brrtrouter_canon = brrtrouter_abs.canonicalize().unwrap_or(brrtrouter_abs);

        // Calculate relative path from base to brrtrouter_base
        // base is typically a subdirectory of brrtrouter_base (e.g., examples/pet_store is under BRRTRouter root)
        let rel_path = if let Ok(rel) = base_canon.strip_prefix(&brrtrouter_canon) {
            // base is a subdirectory of brrtrouter_base (normal case)
            // Count depth and build ../.. path
            let depth = rel.components().count();
            let mut rel_path = PathBuf::new();
            for _ in 0..depth {
                rel_path.push("..");
            }
            rel_path
        } else if let Ok(rel) = brrtrouter_canon.strip_prefix(&base_canon) {
            // brrtrouter_base is a subdirectory of base (unusual, but handle it)
            rel.to_path_buf()
        } else {
            // No direct relationship - calculate manually
            // Find common prefix
            let base_parts: Vec<_> = base_canon.components().collect();
            let brrtrouter_parts: Vec<_> = brrtrouter_canon.components().collect();
            let mut common_len = 0;
            let min_len = base_parts.len().min(brrtrouter_parts.len());
            for i in 0..min_len {
                if base_parts[i] == brrtrouter_parts[i] {
                    common_len += 1;
                } else {
                    break;
                }
            }
            // Build path: go up (base_depth - common_len) times, then down (brrtrouter_parts - common_len)
            let mut rel_path = PathBuf::new();
            let up_levels = base_parts.len() - common_len;
            for _ in 0..up_levels {
                rel_path.push("..");
            }
            for part in brrtrouter_parts.iter().skip(common_len) {
                rel_path.push(part);
            }
            rel_path
        };

        let rel_path_str = rel_path.to_string_lossy().to_string();
        let macros_path = rel_path.join("brrtrouter_macros");
        let macros_path_str = macros_path.to_string_lossy().to_string();

        // Debug: log the calculated paths
        eprintln!(
            "DEBUG: base={:?}, brrtrouter_base={:?}, rel_path={:?}, rel_path_str={:?}",
            base, &brrtrouter_base_clone, rel_path, rel_path_str
        );

        (rel_path_str, macros_path_str)
    };

    let version_str = version.unwrap_or_else(|| "0.1.0".to_string());
    let rendered = CargoTomlTemplateData {
        name: slug.to_string(),
        version: version_str,
        use_workspace_deps,
        brrtrouter_path,
        brrtrouter_macros_path,
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
    write_main_rs_with_options(dir, slug, routes, false)
}

/// Write the main.rs entry point with options
///
/// # Arguments
///
/// * `dir` - Output directory (typically `src/`)
/// * `slug` - Project name slug
/// * `routes` - All routes from the OpenAPI spec
/// * `use_crate_prefix` - If true, use `crate::registry`, else use `{{ name }}::registry`
pub fn write_main_rs_with_options(
    dir: &Path,
    slug: &str,
    routes: Vec<RouteMeta>,
    use_crate_prefix: bool,
) -> anyhow::Result<()> {
    let routes = routes
        .into_iter()
        .map(|r| RouteDisplay {
            method: r.method.to_string(),
            // JSF P0-2: Convert Arc<str> to String for template
            path: r.path_pattern.to_string(),
            handler: r.handler_name.to_string(),
        })
        .collect();
    let rendered = MainRsTemplateData {
        name: slug.to_string(),
        routes,
        use_crate_prefix,
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

/// Template data for generating implementation controller stubs
#[derive(Template)]
#[template(path = "impl_controller_stub.rs.txt", escape = "none")]
pub struct ImplControllerStubTemplateData {
    /// Handler function name
    pub handler_name: String,
    /// Controller struct name
    pub struct_name: String,
    /// Generated crate name (e.g., "bff")
    pub crate_name: String,
    /// Request struct fields
    pub request_fields: Vec<FieldDef>,
    /// Response struct fields
    pub response_fields: Vec<FieldDef>,
    /// Types to import
    pub imports: Vec<String>,
    /// Whether this handler uses Server-Sent Events
    pub sse: bool,
    /// Whether the response is an array
    pub response_is_array: bool,
    /// Array literal for response (if array)
    pub response_array_literal: String,
    /// Whether an example response is available
    pub has_example: bool,
    /// Example response as JSON string
    pub example_json: String,
}

/// Template data for generating implementation crate Cargo.toml
#[derive(Template)]
#[template(path = "impl_cargo.toml.txt", escape = "none")]
pub struct ImplCargoTomlTemplateData {
    /// Implementation crate name (e.g., "bff_impl")
    pub impl_crate_name: String,
    /// Generated crate name (e.g., "bff")
    pub crate_name: String,
    /// Whether to use workspace dependencies
    pub use_workspace_deps: bool,
    /// Relative path to BRRTRouter (only used when use_workspace_deps is false)
    pub brrtrouter_path: String,
    /// Relative path to brrtrouter_macros (only used when use_workspace_deps is false)
    pub brrtrouter_macros_path: String,
}

/// Template data for generating implementation crate main.rs
#[derive(Template)]
#[template(path = "impl_main.rs.txt", escape = "none")]
pub struct ImplMainRsTemplateData {
    /// Generated crate name (e.g., "bff")
    pub crate_name: String,
    /// Routes for displaying in comments
    pub routes: Vec<RouteDisplay>,
}

/// Write an implementation controller stub file
///
/// Creates a starting point for user implementation.
/// Stubs are NOT auto-regenerated - user must use --force to overwrite.
pub fn write_impl_controller_stub(params: ImplControllerStubParams) -> anyhow::Result<()> {
    if params.path.exists() && !params.force {
        return Ok(()); // Already handled in generate_impl_stubs
    }

    // Extract example data if available
    let example_map = params
        .example
        .as_ref()
        .and_then(|v| match v {
            Value::Object(map) => Some(map.clone()),
            _ => None,
        })
        .unwrap_or_default();

    // Enrich response fields with example data
    let enriched_fields = params
        .res_fields
        .iter()
        .map(|field| {
            // Use original_name for lookup (JSON key) but name for code generation (Rust identifier)
            let value = example_map
                .get(&field.original_name) // Look up by original JSON name (e.g., "type" not "r#type")
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

    // Detect if response is an array
    let response_is_array = params.res_fields.len() == 1 && params.res_fields[0].name == "items";

    // Generate array literal if needed
    let response_array_literal = if response_is_array {
        if let Some(ref ex) = params.example {
            if ex.is_array() {
                let items_field = FieldDef {
                    name: "items".to_string(),
                    original_name: "items".to_string(),
                    ty: params.res_fields[0].ty.clone(),
                    optional: false,
                    value: String::new(),
                };
                rust_literal_for_example(&items_field, ex)
            } else {
                "vec![]".to_string()
            }
        } else {
            "vec![]".to_string()
        }
    } else {
        String::new()
    };

    let example_json = params
        .example
        .as_ref()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
        .unwrap_or_default();

    let stub_data = ImplControllerStubTemplateData {
        handler_name: params.handler.to_string(),
        struct_name: params.struct_name.to_string(),
        crate_name: params.crate_name.to_string(),
        request_fields: params.req_fields.to_vec(),
        response_fields: enriched_fields,
        imports: params.imports.iter().cloned().collect(),
        sse: params.sse,
        response_is_array,
        response_array_literal,
        has_example: params.example.is_some(),
        example_json,
    };

    let rendered = stub_data.render()?;
    fs::write(params.path, rendered)?;
    println!("✅ Generated implementation stub: {:?}", params.path);

    Ok(())
}

/// Write implementation crate Cargo.toml
pub fn write_impl_cargo_toml(impl_output_dir: &Path, component_name: &str) -> anyhow::Result<()> {
    let cargo_toml_path = impl_output_dir.join("Cargo.toml");

    // Detect workspace context - check parent directories for workspace Cargo.toml
    let mut use_workspace_deps = false;
    let mut current = impl_output_dir;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    use_workspace_deps = true;
                    break;
                }
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    // Calculate relative paths if not using workspace deps
    let (brrtrouter_path, brrtrouter_macros_path) = if use_workspace_deps {
        (String::new(), String::new())
    } else {
        // Try to find BRRTRouter by looking for its Cargo.toml in parent directories
        let mut current = impl_output_dir;
        let mut brrtrouter_root = None;
        loop {
            let cargo_toml = current.join("Cargo.toml");
            if cargo_toml.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("name = \"brrtrouter\"") {
                        brrtrouter_root = Some(current.to_path_buf());
                        break;
                    }
                }
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        if let Some(brrtrouter_base) = brrtrouter_root {
            // Calculate relative path manually
            let mut relative = PathBuf::new();
            let mut current = impl_output_dir;
            while let Some(parent) = current.parent() {
                if parent == brrtrouter_base {
                    break;
                }
                relative.push("..");
                current = parent;
            }
            let base_str = if relative.as_os_str().is_empty() {
                "../BRRTRouter".to_string()
            } else {
                relative.join("BRRTRouter").to_string_lossy().to_string()
            };
            (base_str.clone(), format!("{}/brrtrouter_macros", base_str))
        } else {
            (
                "../BRRTRouter".to_string(),
                "../BRRTRouter/brrtrouter_macros".to_string(),
            )
        }
    };

    let template_data = ImplCargoTomlTemplateData {
        impl_crate_name: format!("{}_impl", component_name),
        crate_name: component_name.to_string(),
        use_workspace_deps,
        brrtrouter_path,
        brrtrouter_macros_path,
    };

    let rendered = template_data.render()?;
    fs::write(&cargo_toml_path, rendered)?;

    Ok(())
}

/// Write implementation crate main.rs
pub fn write_impl_main_rs(
    impl_src_dir: &Path,
    component_name: &str,
    routes: &[RouteMeta],
) -> anyhow::Result<()> {
    let main_rs_path = impl_src_dir.join("main.rs");

    let route_displays: Vec<RouteDisplay> = routes
        .iter()
        .map(|r| RouteDisplay {
            method: format!("{:?}", r.method),
            // JSF P0-2: Convert Arc<str> to String for template
            path: r.path_pattern.to_string(),
            handler: r.handler_name.to_string(),
        })
        .collect();

    let template_data = ImplMainRsTemplateData {
        crate_name: component_name.to_string(),
        routes: route_displays,
    };

    let rendered = template_data.render()?;
    fs::write(&main_rs_path, rendered)?;

    Ok(())
}

/// Update impl crate's controllers/mod.rs to include a new module
///
/// If mod.rs doesn't exist, create it with the module declaration.
/// If it exists, add the module declaration if not already present.
/// Use --force to overwrite existing declarations.
pub fn update_impl_mod_rs(
    controllers_dir: &Path,
    handler: &str,
    force: bool,
) -> anyhow::Result<()> {
    let mod_rs_path = controllers_dir.join("mod.rs");

    if !mod_rs_path.exists() {
        // Create new mod.rs
        let content = format!(
            "// Controller module declarations\n// This file is automatically updated when stubs are generated\n// You can manually add/remove module declarations as needed\n\npub mod {};\n",
            handler
        );
        fs::write(&mod_rs_path, content)?;
        return Ok(());
    }

    // Read existing mod.rs
    let content = fs::read_to_string(&mod_rs_path)?;
    let module_decl = format!("pub mod {};", handler);

    // Check if module already declared
    if content.contains(&module_decl) {
        if force {
            // Force mode: replace existing declaration (in case handler name changed)
            let new_content = content.replace(&format!("pub mod {};", handler), &module_decl);
            fs::write(&mod_rs_path, new_content)?;
        }
        // Already exists, no need to add
        return Ok(());
    }

    // Add module declaration
    let new_content = if content.trim().is_empty() {
        format!(
            "// Controller module declarations\n// This file is automatically updated when stubs are generated\n\n{}\n",
            module_decl
        )
    } else {
        format!("{}\n{}", content.trim_end(), module_decl)
    };

    fs::write(&mod_rs_path, new_content)?;
    Ok(())
}
