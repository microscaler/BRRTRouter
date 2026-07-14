//! Generated impl crate registration (Tier 1 — Fix A).
//!
//! Discovers `impl/src/controllers/*.rs` on disk, validates against OpenAPI
//! `x-brrtrouter-impl`, and renders `impl/src/impl_registry.rs`.

use askama::Template;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use super::schema::to_camel_case;
use super::stack_size::compute_stack_size;
use crate::spec::RouteMeta;

/// One impl controller wired into generated `impl_registry.rs`.
#[derive(Debug, Clone)]
pub struct ImplRegistryEntry {
    /// Handler / module name (`operationId`, snake_case).
    pub name: String,
    /// Typed controller struct or untyped handler function name.
    pub controller_struct: String,
    /// Coroutine stack size in bytes.
    pub stack_size_bytes: usize,
    /// Uses `HandlerRequest` / `HandlerResponse` instead of `#[handler]`.
    pub is_untyped: bool,
}

/// Outcome of planning impl registration without writing files.
#[derive(Debug, Default)]
pub struct ImplRegistryPlan {
    pub registry_entries: Vec<ImplRegistryEntry>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "impl_registry.rs.txt", escape = "none")]
struct ImplRegistryTemplateData {
    entries: Vec<ImplRegistryEntry>,
    controller_prefix: String,
}

/// List controller module names from `impl/src/controllers/*.rs` (sorted).
pub fn discover_impl_controllers(controllers_dir: &Path) -> anyhow::Result<Vec<String>> {
    if !controllers_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in fs::read_dir(controllers_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem == "mod" || stem.starts_with('.') {
            continue;
        }
        names.push(stem.to_string());
    }
    names.sort();
    Ok(names)
}

/// Detect typed vs untyped controller and resolve the struct name for typed handlers.
pub fn detect_controller_kind(content: &str, handler: &str) -> (bool, String) {
    let is_untyped = content.contains("HandlerRequest")
        && content.contains("HandlerResponse")
        && !content.contains("#[handler");

    if is_untyped {
        let fn_name = if content.contains("pub fn handle_untyped") {
            "handle_untyped"
        } else {
            "handle"
        };
        return (true, fn_name.to_string());
    }

    if let Some(idx) = content.find("#[handler(") {
        let rest = &content[idx + "#[handler(".len()..];
        if let Some(end) = rest.find(')') {
            let struct_name = rest[..end].trim();
            if !struct_name.is_empty() {
                return (false, struct_name.to_string());
            }
        }
    }

    (false, format!("{}Controller", to_camel_case(handler)))
}

/// Build registry entries for migration: prefer explicit main.rs match arms when present.
pub fn plan_impl_registry_for_migration(
    routes: &[RouteMeta],
    controllers_dir: &Path,
    main_handlers: &BTreeSet<String>,
) -> anyhow::Result<ImplRegistryPlan> {
    let mut plan = plan_impl_registry(routes, controllers_dir)?;

    if main_handlers.is_empty() {
        return Ok(plan);
    }

    plan.registry_entries
        .retain(|e| main_handlers.contains(&e.name));

    plan.warnings
        .retain(|w| main_handlers.iter().any(|h| warning_mentions_handler(w, h)));

    // Drop x-brrtrouter-impl errors for handlers not being wired in this migration.
    plan.errors
        .retain(|e| main_handlers.iter().any(|h| error_mentions_handler(e, h)));

    Ok(plan)
}

fn error_mentions_handler(err: &str, handler: &str) -> bool {
    err.contains(&format!("handler '{handler}'"))
}

fn warning_mentions_handler(warn: &str, handler: &str) -> bool {
    warn.contains(&format!("'{handler}'"))
}

/// Build registry entries and collect validation warnings/errors.
pub fn plan_impl_registry(
    routes: &[RouteMeta],
    controllers_dir: &Path,
) -> anyhow::Result<ImplRegistryPlan> {
    let discovered = discover_impl_controllers(controllers_dir)?;
    let discovered_set: BTreeSet<_> = discovered.iter().cloned().collect();
    let routes_by_handler: HashMap<&str, &RouteMeta> = routes
        .iter()
        .map(|r| (r.handler_name.as_ref(), r))
        .collect();

    let mut plan = ImplRegistryPlan::default();

    for route in routes {
        if route.x_brrtrouter_impl == Some(true) {
            let stub_path = controllers_dir.join(format!("{}.rs", route.handler_name));
            if !stub_path.is_file() {
                plan.errors.push(format!(
                    "x-brrtrouter-impl: true for handler '{}' but no impl file at {}",
                    route.handler_name,
                    stub_path.display()
                ));
            }
        }
    }

    for handler in &discovered {
        let stub_path = controllers_dir.join(format!("{handler}.rs"));
        let content = fs::read_to_string(&stub_path).unwrap_or_default();
        let route = routes_by_handler.get(handler.as_str());

        if let Some(route) = route {
            if route.x_brrtrouter_impl == Some(false) {
                plan.warnings.push(format!(
                    "impl file '{}' exists but x-brrtrouter-impl: false — registry will still wire it; set x-brrtrouter-impl: true or remove the file",
                    handler
                ));
            } else if route.x_brrtrouter_impl.is_none() {
                plan.warnings.push(format!(
                    "impl file '{}' exists but x-brrtrouter-impl is omitted (legacy) — consider setting x-brrtrouter-impl: true",
                    handler
                ));
            }
        } else {
            plan.warnings.push(format!(
                "orphan impl controller '{}' — no matching OpenAPI operationId; registry arm will never dispatch",
                handler
            ));
        }

        let (is_untyped, controller_struct) = detect_controller_kind(&content, handler);
        let stack_size_bytes = route
            .map(|r| compute_stack_size(*r))
            .unwrap_or(20_480);

        plan.registry_entries.push(ImplRegistryEntry {
            name: handler.clone(),
            controller_struct,
            stack_size_bytes,
            is_untyped,
        });
    }

    for route in routes {
        if route.x_brrtrouter_impl == Some(true)
            && !discovered_set.contains(route.handler_name.as_ref())
        {
            // Already captured in errors loop above; keep for clarity if errors deduped later.
        }
    }

    Ok(plan)
}

/// Fully regenerate `impl/src/controllers/mod.rs` from discovered controller files.
pub fn regenerate_impl_mod_rs(controllers_dir: &Path) -> anyhow::Result<()> {
    let handlers = discover_impl_controllers(controllers_dir)?;
    let mod_rs_path = controllers_dir.join("mod.rs");

    let mut content = String::from(
        "// ⚠️ WARNING: This file is auto-generated by BRRTRouter\n\
         // ⚠️ DO NOT MODIFY - Changes will be overwritten on next generation\n\n",
    );
    for handler in &handlers {
        content.push_str(&format!("pub mod {handler};\n"));
    }

    fs::write(&mod_rs_path, content)?;
    println!("✅ Regenerated impl controllers/mod.rs → {mod_rs_path:?} ({})", handlers.len());
    Ok(())
}

/// Write `impl_registry.rs` from a prepared plan (used by migrate-registration).
pub fn write_impl_registry_from_plan(
    impl_src_dir: &Path,
    plan: &ImplRegistryPlan,
    regenerate_mod_rs: bool,
) -> anyhow::Result<()> {
    if !plan.errors.is_empty() {
        for err in &plan.errors {
            eprintln!("[impl-registry][error] {err}");
        }
        anyhow::bail!(
            "impl registry validation failed ({} error(s))",
            plan.errors.len()
        );
    }

    for warn in &plan.warnings {
        eprintln!("[impl-registry][warning] {warn}");
    }

    let controllers_dir = impl_src_dir.join("controllers");
    if regenerate_mod_rs {
        regenerate_impl_mod_rs(&controllers_dir)?;
    }

    let registry_path = impl_src_dir.join("impl_registry.rs");
    let controller_prefix = controller_module_prefix(impl_src_dir);
    let rendered = ImplRegistryTemplateData {
        entries: plan.registry_entries.clone(),
        controller_prefix,
    }
    .render()?;
    fs::write(&registry_path, rendered)?;

    let legacy = impl_src_dir.join("registry.rs");
    if legacy.exists() {
        fs::remove_file(&legacy)?;
        println!("ℹ️  Removed legacy impl registry.rs (use impl_registry.rs)");
    }

    println!(
        "✅ Generated impl_registry.rs → {registry_path:?} ({} controller(s))",
        plan.registry_entries.len()
    );

    Ok(())
}

fn controller_module_prefix(impl_src_dir: &Path) -> String {
    let lib_path = impl_src_dir.join("lib.rs");
    let has_lib_controllers = lib_path.is_file()
        && fs::read_to_string(&lib_path)
            .map(|c| c.contains("pub mod controllers"))
            .unwrap_or(false);
    if !has_lib_controllers {
        return "crate::controllers".to_string();
    }
    let impl_dir = impl_src_dir.parent().unwrap_or(impl_src_dir);
    if let Ok(cargo) = fs::read_to_string(impl_dir.join("Cargo.toml")) {
        for line in cargo.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("name = ") {
                if let Some(name) = trimmed
                    .trim_start_matches("name = ")
                    .trim()
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                {
                    return format!("{name}::controllers");
                }
            }
        }
    }
    "crate::controllers".to_string()
}

/// Write `impl/src/impl_registry.rs` from disk discovery + OpenAPI validation.
pub fn write_impl_registry_rs(
    impl_src_dir: &Path,
    routes: &[RouteMeta],
) -> anyhow::Result<ImplRegistryPlan> {
    regen_impl_registry_from_routes(impl_src_dir, routes, true, false)
}

/// Regenerate only `impl_registry.rs` from full disk discovery (never touches controller bodies).
///
/// Use after adding a new `impl/src/controllers/<operationId>.rs` when migration scoping
/// omitted it (e.g. `save_draft_quote` on bidding). Default: do not regen `controllers/mod.rs`
/// so hand-edited module headers are preserved.
pub fn regen_impl_registry_from_routes(
    impl_src_dir: &Path,
    routes: &[RouteMeta],
    regen_mod_rs: bool,
    dry_run: bool,
) -> anyhow::Result<ImplRegistryPlan> {
    let controllers_dir = impl_src_dir.join("controllers");
    let plan = plan_impl_registry(routes, &controllers_dir)?;

    if dry_run {
        print_impl_registry_plan(&plan, &controllers_dir);
        if !plan.errors.is_empty() {
            anyhow::bail!(
                "impl registry validation failed ({} error(s))",
                plan.errors.len()
            );
        }
        return Ok(plan);
    }

    write_impl_registry_from_plan(impl_src_dir, &plan, regen_mod_rs)?;
    Ok(plan)
}

/// Load spec and regen `impl/src/impl_registry.rs` for an impl crate directory.
pub fn regen_impl_registry(
    spec_path: &Path,
    impl_output_dir: &Path,
    regen_mod_rs: bool,
    dry_run: bool,
) -> anyhow::Result<ImplRegistryPlan> {
    let spec_str = spec_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in spec path"))?;
    let (routes, _slug) = crate::spec::load_spec(spec_str)?;
    let impl_src_dir = impl_output_dir.join("src");
    regen_impl_registry_from_routes(&impl_src_dir, &routes, regen_mod_rs, dry_run)
}

/// Print a human-readable plan summary (dry-run).
pub fn print_impl_registry_plan(plan: &ImplRegistryPlan, controllers_dir: &Path) {
    println!("=== impl registry plan ===");
    println!("controllers_dir: {}", controllers_dir.display());
    println!("register: {} controller(s)", plan.registry_entries.len());
    for entry in &plan.registry_entries {
        let kind = if entry.is_untyped {
            "untyped"
        } else {
            "typed"
        };
        println!(
            "  - {} ({kind}, stack={})",
            entry.name, entry.stack_size_bytes
        );
    }
    if !plan.warnings.is_empty() {
        println!("warnings: {}", plan.warnings.len());
        for w in &plan.warnings {
            println!("  ⚠️  {w}");
        }
    }
    if !plan.errors.is_empty() {
        println!("errors: {}", plan.errors.len());
        for e in &plan.errors {
            println!("  ❌ {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::RouteMeta;
    use http::Method;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn route(handler: &str, impl_flag: Option<bool>) -> RouteMeta {
        RouteMeta {
            method: Method::GET,
            path_pattern: "/test".into(),
            handler_name: handler.into(),
            parameters: vec![],
            request_schema: None,
            request_body_required: false,
            request_content_types: Vec::new(),
            response_schema: None,
            example: None,
            responses: HashMap::new(),
            security: vec![],
            example_name: String::new(),
            project_slug: String::new(),
            output_dir: PathBuf::new(),
            base_path: String::new(),
            sse: false,
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
            cors_policy: crate::middleware::RouteCorsPolicy::Inherit,
            x_service: None,
            x_brrtrouter_downstream_path: None,
            x_brrtrouter_impl: impl_flag,
        }
    }

    #[test]
    fn detect_typed_controller_struct_from_handler_attribute() {
        let content = "#[handler(ListVehiclesController)]\npub fn handle(_req: TypedHandlerRequest<Request>) -> Response {}";
        let (untyped, name) = detect_controller_kind(content, "list_vehicles");
        assert!(!untyped);
        assert_eq!(name, "ListVehiclesController");
    }

    #[test]
    fn detect_untyped_controller() {
        let content =
            "pub fn handle(req: HandlerRequest) -> HandlerResponse { HandlerResponse::empty() }";
        let (untyped, name) = detect_controller_kind(content, "create_organization");
        assert!(untyped);
        assert_eq!(name, "handle");
    }

    #[test]
    fn plan_errors_when_impl_required_but_file_missing() {
        let dir = std::env::temp_dir().join(format!(
            "impl_reg_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        let plan = plan_impl_registry(&[route("get_foo", Some(true))], &dir).unwrap();
        assert_eq!(plan.registry_entries.len(), 0);
        assert!(!plan.errors.is_empty());
        assert!(plan.errors[0].contains("get_foo"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn regen_dry_run_includes_all_disk_controllers() {
        let base = std::env::temp_dir().join(format!(
            "impl_regen_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let impl_src = base.join("src");
        let controllers = impl_src.join("controllers");
        fs::create_dir_all(&controllers).unwrap();

        for name in ["accept_quote", "save_draft_quote"] {
            fs::write(
                controllers.join(format!("{name}.rs")),
                format!("#[handler({name}Controller)]\npub fn handle(_req: TypedHandlerRequest<Request>) -> Response {{}}"),
            )
            .unwrap();
        }

        let routes = vec![
            route("accept_quote", Some(true)),
            route("save_draft_quote", Some(true)),
        ];

        let plan =
            regen_impl_registry_from_routes(&impl_src, &routes, false, true).unwrap();
        assert_eq!(plan.registry_entries.len(), 2);
        assert!(
            plan.registry_entries
                .iter()
                .any(|e| e.name == "save_draft_quote")
        );

        let _ = fs::remove_dir_all(&base);
    }
}
