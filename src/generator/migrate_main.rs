//! `migrate-main` — collapse legacy impl `main.rs` to Fix B `RunAppBuilder` shape.

use std::fs;
use std::path::{Path, PathBuf};

/// Outcome of analyzing a legacy impl `main.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainMigrationPlan {
    pub package_name: String,
    pub gen_crate_ident: String,
    pub default_port: u16,
    pub service_name: String,
    pub has_mod_controllers: bool,
    pub has_lifeguard_prometheus: bool,
    pub has_db_warm: bool,
    pub spec_default: String,
    pub doc_dir_default: String,
    pub config_default: String,
    pub already_migrated: bool,
}

/// Options for `migrate_main`.
#[derive(Debug, Clone)]
pub struct MigrateMainOptions {
    pub impl_output_dir: PathBuf,
    pub apply: bool,
    pub default_port: Option<u16>,
    pub service_name: Option<String>,
}

/// Analyze legacy `main.rs` and optionally rewrite to `RunAppBuilder`.
pub fn migrate_main(opts: &MigrateMainOptions) -> anyhow::Result<MainMigrationPlan> {
    let main_path = opts.impl_output_dir.join("src").join("main.rs");
    let cargo_path = opts.impl_output_dir.join("Cargo.toml");
    let content = fs::read_to_string(&main_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", main_path.display()))?;
    let cargo = fs::read_to_string(&cargo_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", cargo_path.display()))?;

    let impl_registry_path = opts.impl_output_dir.join("src").join("impl_registry.rs");
    let impl_registry_content = fs::read_to_string(&impl_registry_path).unwrap_or_default();

    let mut plan = analyze_main_rs(&content, &cargo, &impl_registry_content)?;
    if let Some(port) = opts.default_port {
        plan.default_port = port;
    }
    if let Some(name) = &opts.service_name {
        plan.service_name = name.clone();
    }

    print_main_migration_plan(&plan, &main_path);

    if plan.already_migrated {
        println!("ℹ️  main.rs already uses RunAppBuilder — no changes");
        return Ok(plan);
    }

    if opts.apply {
        let rendered = render_run_app_main(&plan);
        let line_count = rendered.lines().count();
        fs::write(&main_path, &rendered)?;
        println!(
            "✅ Wrote RunApp main → {} ({line_count} lines)",
            main_path.display(),
        );
    } else {
        println!("ℹ️  dry-run — pass --apply to write main.rs");
    }

    Ok(plan)
}

pub fn analyze_main_rs(
    main_content: &str,
    cargo_toml: &str,
    impl_registry_content: &str,
) -> anyhow::Result<MainMigrationPlan> {
    let package_name = read_cargo_package_name(cargo_toml)?;
    let gen_crate_ident = format!("{package_name}_gen");
    let lib_controller_prefix = format!("{package_name}::controllers");
    let uses_lib_controllers = impl_registry_content.contains(&lib_controller_prefix);

    Ok(MainMigrationPlan {
        package_name: package_name.clone(),
        gen_crate_ident,
        default_port: extract_default_port(main_content).unwrap_or(8081),
        service_name: extract_service_name(main_content).unwrap_or_else(|| package_name.clone()),
        has_mod_controllers: !uses_lib_controllers
            && (main_content.contains("mod controllers;")
                || impl_registry_content.contains("crate::controllers::")),
        has_lifeguard_prometheus: main_content.contains("set_extra_prometheus")
            && main_content.contains("lifeguard::metrics"),
        has_db_warm: main_content.contains("hauliage_database::db()"),
        spec_default: extract_clap_default(main_content, "spec")
            .unwrap_or_else(|| "../gen/doc/openapi.yaml".into()),
        doc_dir_default: extract_clap_default(main_content, "doc_dir")
            .unwrap_or_else(|| "../gen/doc".into()),
        config_default: extract_clap_default(main_content, "config")
            .unwrap_or_else(|| "./config/config.yaml".into()),
        already_migrated: main_content.contains("RunAppBuilder"),
    })
}

fn read_cargo_package_name(cargo_toml: &str) -> anyhow::Result<String> {
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name = ") {
            if let Some(name) = trimmed
                .trim_start_matches("name = ")
                .trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
            {
                return Ok(name.to_string());
            }
        }
    }
    anyhow::bail!("Could not find [package].name in Cargo.toml")
}

fn extract_default_port(content: &str) -> Option<u16> {
    for line in content.lines() {
        if let Some(idx) = line.rfind(".unwrap_or(") {
            let rest = &line[idx + ".unwrap_or(".len()..];
            let inner = rest.trim_end_matches(");").trim_end_matches(')');
            if let Ok(port) = inner.parse::<u16>() {
                return Some(port);
            }
        }
    }
    None
}

fn extract_service_name(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.contains("example server listening") {
            if let Some(start) = line.find('🚀') {
                let rest = &line[start + '🚀'.len_utf8()..];
                let name = rest
                    .trim()
                    .trim_start_matches('"')
                    .split(" example server")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches('"')
                    .to_string();
                if !name.is_empty() && !name.contains('{') {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn extract_clap_default(content: &str, field: &str) -> Option<String> {
    let needle = format!("{field}:");
    for line in content.lines() {
        if line.contains(&needle) && line.contains("default_value") {
            if let Some(v) = extract_quoted_arg_value(line, "default_value = ") {
                return Some(v);
            }
            if line.contains("default_value_t") {
                continue;
            }
        }
    }
    None
}

fn extract_quoted_arg_value(line: &str, prefix: &str) -> Option<String> {
    let idx = line.find(prefix)?;
    let rest = &line[idx + prefix.len()..];
    let rest = rest.trim();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub fn render_run_app_main(plan: &MainMigrationPlan) -> String {
    let gen = &plan.gen_crate_ident;
    let hooks_block = render_hooks_block(plan);
    let mod_controllers = if plan.has_mod_controllers {
        "mod controllers;\n"
    } else {
        ""
    };

    format!(
        r#"// {pkg} impl binary — business logic in impl/src/controllers/
#![allow(clippy::uninlined_format_args)]

{mod_controllers}mod impl_registry;

use brrtrouter::server::{{RunAppArgs, RunAppBuilder{hooks_import}}};
use clap::Parser;
use {gen}::registry as gen_registry;
use std::io;
use std::path::PathBuf;
{arc_import}
#[cfg(feature = "jemalloc")]
use tikv_jemallocator::Jemalloc;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Parser)]
struct Args {{
    #[arg(short, long, default_value = "{spec_default}")]
    spec: PathBuf,
    #[arg(long)]
    static_dir: Option<PathBuf>,
    #[arg(long, default_value = "{doc_dir_default}")]
    doc_dir: PathBuf,
    #[arg(long, default_value_t = false)]
    hot_reload: bool,
    #[arg(long)]
    test_api_key: Option<String>,
    #[arg(long, default_value = "{config_default}")]
    config: PathBuf,
}}

fn main() -> io::Result<()> {{
    let args = Args::parse();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    RunAppBuilder::new()
        .args(RunAppArgs {{
            spec: args.spec,
            config: args.config,
            doc_dir: args.doc_dir,
            static_dir: args.static_dir,
            hot_reload: args.hot_reload,
            test_api_key: args.test_api_key,
            manifest_dir,
            default_port: {default_port},
            service_name: "{service_name}".into(),
        }})
{hooks_block}        .register(|dispatcher, routes| unsafe {{
            gen_registry::register_from_spec(dispatcher, routes);
            impl_registry::register_impl(dispatcher, routes);
        }})
        .run()
}}
"#,
        pkg = plan.package_name,
        mod_controllers = mod_controllers,
        hooks_import = if hooks_block.is_empty() {
            String::new()
        } else {
            ", RunAppHooks".to_string()
        },
        gen = gen,
        arc_import = if plan.has_lifeguard_prometheus {
            "use std::sync::Arc;\n".to_string()
        } else {
            String::new()
        },
        spec_default = plan.spec_default,
        doc_dir_default = plan.doc_dir_default,
        config_default = plan.config_default,
        default_port = plan.default_port,
        service_name = plan.service_name,
        hooks_block = hooks_block,
    )
}

fn render_hooks_block(plan: &MainMigrationPlan) -> String {
    if !plan.has_lifeguard_prometheus && !plan.has_db_warm {
        return String::new();
    }

    let mut lines = vec!["        .hooks(RunAppHooks {".to_string()];
    if plan.has_lifeguard_prometheus {
        lines.push(
            "            extra_prometheus: Some(Arc::new(lifeguard::metrics::prometheus_scrape_text)),".to_string(),
        );
    }
    if plan.has_db_warm {
        lines.push("            before_listen: Some(Box::new(|| {".to_string());
        lines.push("                let _ = hauliage_database::db();".to_string());
        lines.push("            })),".to_string());
    }
    lines.push("            ..Default::default()".to_string());
    lines.push("        })".to_string());
    format!("{}\n", lines.join("\n"))
}

pub fn print_main_migration_plan(plan: &MainMigrationPlan, main_path: &Path) {
    println!("=== migrate-main plan ===");
    println!("main: {}", main_path.display());
    println!("package: {}", plan.package_name);
    println!("gen crate: {}", plan.gen_crate_ident);
    println!("default_port: {}", plan.default_port);
    println!("service_name: {}", plan.service_name);
    println!("mod controllers: {}", plan.has_mod_controllers);
    println!("lifeguard prometheus: {}", plan.has_lifeguard_prometheus);
    println!("db warm: {}", plan.has_db_warm);
    println!("already_migrated: {}", plan.already_migrated);
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_MAIN: &str = r#"
mod controllers;
use foo_gen::registry as gen_registry;
fn main() {
    let port = app_config.port.or_else(|| None).unwrap_or(8017);
    println!("🚀 hauliage_customs example server listening on {addr}");
    service.set_extra_prometheus(Some(std::sync::Arc::new(|| {
        lifeguard::metrics::prometheus_scrape_text()
    })));
    let _ = hauliage_database::db();
}
"#;

    #[test]
    fn analyze_detects_port_hooks_and_banner() {
        let cargo = "[package]\nname = \"hauliage_customs\"\n";
        let plan = analyze_main_rs(LEGACY_MAIN, cargo, "").unwrap();
        assert_eq!(plan.default_port, 8017);
        assert_eq!(plan.service_name, "hauliage_customs");
        assert!(plan.has_lifeguard_prometheus);
        assert!(plan.has_db_warm);
        assert!(plan.has_mod_controllers);
        assert_eq!(plan.gen_crate_ident, "hauliage_customs_gen");
    }

    #[test]
    fn render_includes_run_app_builder() {
        let plan = MainMigrationPlan {
            package_name: "hauliage_telemetry".into(),
            gen_crate_ident: "hauliage_telemetry_gen".into(),
            default_port: 8080,
            service_name: "geographic_telemetry_and_mapping".into(),
            has_mod_controllers: true,
            has_lifeguard_prometheus: true,
            has_db_warm: true,
            spec_default: "../gen/doc/openapi.yaml".into(),
            doc_dir_default: "../gen/doc".into(),
            config_default: "./config/config.yaml".into(),
            already_migrated: false,
        };
        let rendered = render_run_app_main(&plan);
        assert!(rendered.contains("RunAppBuilder"));
        assert!(rendered.contains("geographic_telemetry_and_mapping"));
        assert!(rendered.contains("lifeguard::metrics::prometheus_scrape_text"));
        assert!(rendered.contains("hauliage_database::db()"));
    }

    #[test]
    fn cargo_ident_hyphenated() {
        use crate::generator::cargo_pkg_name_to_rust_ident;
        assert_eq!(
            cargo_pkg_name_to_rust_ident("market-data_api"),
            "market_data_api"
        );
    }
}
