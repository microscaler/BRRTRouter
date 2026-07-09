//! `migrate-registration` — parity report and optional apply for Tier 1 impl registry migration.
//!
//! Compares manual `main.rs` match arms with disk-discovered controllers and the
//! generated `impl_registry.rs` plan. Can emit `impl_registry.rs` and patch simple
//! `main.rs` registration blocks (single `register_from_spec` + match loop).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::impl_registry::{
    discover_impl_controllers, ImplRegistryPlan,
};

/// Result of comparing main.rs, disk, and planned registry.
#[derive(Debug, Default)]
pub struct MigrationReport {
    pub main_handlers: BTreeSet<String>,
    pub disk_handlers: BTreeSet<String>,
    pub planned_handlers: BTreeSet<String>,
    pub registry_file_handlers: BTreeSet<String>,
    pub already_migrated: bool,
    pub legacy_registry_rs: bool,
    pub complex_main: bool,
    pub complex_main_reason: Option<String>,
    pub parity_ok: Vec<String>,
    pub f5_risk: Vec<String>,
    pub main_orphan_arms: Vec<String>,
    pub disk_not_in_main: Vec<String>,
    pub main_not_on_disk: Vec<String>,
    pub plan: ImplRegistryPlan,
}

/// Options for `migrate_registration`.
#[derive(Debug, Clone)]
pub struct MigrateRegistrationOptions {
    pub spec_path: PathBuf,
    pub impl_output_dir: PathBuf,
    pub component_name: Option<String>,
    /// Write `impl_registry.rs`, regenerate `controllers/mod.rs`, patch `main.rs`.
    pub apply: bool,
    /// Patch `main.rs` even when heuristics flag a complex registration block.
    pub force_main: bool,
}

/// Extract handler names from route registration in `main.rs`.
pub fn extract_main_match_handlers(main_content: &str) -> BTreeSet<String> {
    let mut handlers = BTreeSet::new();
    let mut in_match = false;

    for line in main_content.lines() {
        if let Some(name) = parse_if_route_handler(line) {
            handlers.insert(name);
        }
        if line.contains("match route.handler_name.as_ref()")
            || line.contains("match handler_name.as_ref()")
        {
            in_match = true;
            continue;
        }
        if in_match {
            if let Some(name) = parse_match_arm_handler(line) {
                handlers.insert(name);
            } else if line.trim() == "_ => {}" || line.trim().starts_with("_ =>") {
                in_match = false;
            }
        }
    }

    // Fallback: `"handler" =>` arms only in the registration unsafe block.
    if handlers.is_empty() {
        if let Some(registration) = extract_registration_block(main_content) {
            for line in registration.lines() {
                if let Some(name) = parse_match_arm_handler(line) {
                    if !is_false_positive_handler(&name) {
                        handlers.insert(name);
                    }
                }
            }
        }
    }

    handlers
}

fn is_false_positive_handler(name: &str) -> bool {
    matches!(name, "cookie" | "header" | "query")
}

fn parse_if_route_handler(line: &str) -> Option<String> {
    for needle in [
        "route.handler_name.as_ref() == \"",
        "handler_name.as_ref() == \"",
    ] {
        if let Some(idx) = line.find(needle) {
            let rest = &line[idx + needle.len()..];
            let (name, _) = rest.split_once('"')?;
            if name.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_registration_block(main_content: &str) -> Option<&str> {
    let reg_idx = main_content.find("register_from_spec(&mut dispatcher")?;
    let unsafe_start = main_content[..reg_idx].rfind("unsafe {")?;
    let block_start = unsafe_start + "unsafe {".len();
    let block_end = find_closing_brace(main_content, unsafe_start + "unsafe ".len())?;
    Some(&main_content[block_start..block_end])
}

/// Handler names already listed in `impl_registry.rs` or legacy `registry.rs`.
pub fn extract_registry_file_handlers(content: &str) -> BTreeSet<String> {
    let mut handlers = BTreeSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('"') {
            if let Some((name, _)) = rest.split_once('"') {
                if name.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !name.is_empty() {
                    handlers.insert(name.to_string());
                }
            }
        }
    }
    handlers
}

fn parse_match_arm_handler(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('"')?;
    let (name, after) = rest.split_once('"')?;
    if !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') || name.is_empty() {
        return None;
    }
    let after = after.trim_start();
    if after.starts_with("=>") {
        Some(name.to_string())
    } else {
        None
    }
}

fn find_closing_brace(content: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in content[open_idx..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn count_match_blocks(main_content: &str) -> usize {
    main_content
        .matches("match route.handler_name.as_ref()")
        .count()
}

fn detect_complex_main(main_content: &str) -> Option<String> {
    if main_content.contains("fallback_routes") {
        return None;
    }
    let matches = count_match_blocks(main_content);
    if matches > 1 {
        return Some(format!(
            "{matches} `match route.handler_name` blocks (e.g. company fallback pattern); migrate manually"
        ));
    }
    if main_content.contains("hauliage_company_gen::controllers")
        || main_content.contains("_gen::controllers::")
    {
        return Some("registers gen crate controllers in impl main; migrate manually".into());
    }
    None
}

/// Build migration parity report without writing files.
pub fn analyze_migration(
    routes: &[crate::spec::RouteMeta],
    impl_src_dir: &Path,
    main_content: &str,
) -> anyhow::Result<MigrationReport> {
    let controllers_dir = impl_src_dir.join("controllers");
    let main_handlers = extract_main_match_handlers(main_content);
    let plan = if main_handlers.is_empty() {
        super::impl_registry::plan_impl_registry(routes, &controllers_dir)?
    } else {
        super::impl_registry::plan_impl_registry_for_migration(
            routes,
            &controllers_dir,
            &main_handlers,
        )?
    };
    let disk = discover_impl_controllers(&controllers_dir)?;
    let disk_set: BTreeSet<_> = disk.into_iter().collect();
    let planned: BTreeSet<_> = plan
        .registry_entries
        .iter()
        .map(|e| e.name.clone())
        .collect();

    let impl_reg_path = impl_src_dir.join("impl_registry.rs");
    let legacy_reg_path = impl_src_dir.join("registry.rs");
    let mut registry_file_handlers = BTreeSet::new();
    if impl_reg_path.is_file() {
        registry_file_handlers.extend(extract_registry_file_handlers(&fs::read_to_string(
            &impl_reg_path,
        )?));
    }
    if legacy_reg_path.is_file() {
        registry_file_handlers.extend(extract_registry_file_handlers(&fs::read_to_string(
            &legacy_reg_path,
        )?));
    }

    let already_migrated = main_content.contains("impl_registry::register_impl");
    let legacy_registry_rs = legacy_reg_path.is_file();
    let complex_main_reason = detect_complex_main(main_content);
    let complex_main = complex_main_reason.is_some();

    let mut report = MigrationReport {
        main_handlers: main_handlers.clone(),
        disk_handlers: disk_set.clone(),
        planned_handlers: planned.clone(),
        registry_file_handlers,
        already_migrated,
        legacy_registry_rs,
        complex_main,
        complex_main_reason,
        plan,
        ..Default::default()
    };

    if main_handlers.is_empty() {
        for h in &disk_set {
            if already_migrated || main_handlers.contains(h) {
                report.parity_ok.push(h.clone());
            } else {
                report.f5_risk.push(h.clone());
                report.disk_not_in_main.push(h.clone());
            }
        }
    } else {
        for h in &main_handlers {
            if disk_set.contains(h) {
                report.parity_ok.push(h.clone());
            } else {
                report.main_orphan_arms.push(h.clone());
                report.main_not_on_disk.push(h.clone());
            }
        }
    }

    Ok(report)
}

pub fn print_migration_report(report: &MigrationReport, impl_src_dir: &Path) {
    println!("=== migrate-registration report ===");
    println!("impl_src: {}", impl_src_dir.display());
    println!(
        "status: {}",
        if report.already_migrated {
            "already migrated (impl_registry::register_impl in main.rs)"
        } else if report.complex_main {
            "complex main.rs — manual migration required before --apply"
        } else if report.main_handlers.is_empty() && !report.disk_handlers.is_empty() {
            "F5 risk — controllers on disk but no main.rs match arms"
        } else {
            "ready for Tier 1 migration"
        }
    );

    if report.legacy_registry_rs {
        println!("legacy: impl/src/registry.rs present (will be removed on apply/regen)");
    }

    if let Some(reason) = &report.complex_main_reason {
        println!("complex_main: {reason}");
    }

    println!("counts:");
    println!("  disk controllers: {}", report.disk_handlers.len());
    println!("  main.rs match arms: {}", report.main_handlers.len());
    println!("  planned registry: {}", report.planned_handlers.len());

    if !report.parity_ok.is_empty() {
        println!("parity_ok ({}):", report.parity_ok.len());
        for h in &report.parity_ok {
            println!("  ✅ {h}");
        }
    }
    if !report.f5_risk.is_empty() {
        println!("f5_risk — on disk, not in main match ({}):", report.f5_risk.len());
        for h in &report.f5_risk {
            println!("  ❌ {h}");
        }
    }
    if !report.main_orphan_arms.is_empty() {
        println!("main_orphan — in main match, no controller file ({}):", report.main_orphan_arms.len());
        for h in &report.main_orphan_arms {
            println!("  ⚠️  {h}");
        }
    }

    if !report.plan.warnings.is_empty() {
        println!("plan warnings: {}", report.plan.warnings.len());
        for w in &report.plan.warnings {
            println!("  ⚠️  {w}");
        }
    }
    if !report.plan.errors.is_empty() {
        println!("plan errors: {}", report.plan.errors.len());
        for e in &report.plan.errors {
            println!("  ❌ {e}");
        }
    }
}

/// Patch a **simple** main.rs: alias gen registry, add `mod impl_registry`, replace match loop.
pub fn patch_main_registration_simple(main_content: &str, force: bool) -> anyhow::Result<String> {
    let mut content = fix_gen_registry_import(main_content);
    content = fix_misplaced_impl_registry_mod(&content);

    if !content.contains("mod impl_registry") {
        content = insert_impl_registry_mod(&content);
    }

    if content.contains("impl_registry::register_impl") {
        content = remove_manual_match_registration_loop(&content)?;
        return Ok(content);
    }

    if content.contains("fallback_routes") {
        return patch_fallback_registration(&content);
    }

    if !force && detect_complex_main(main_content).is_some() {
        anyhow::bail!("main.rs uses a complex registration pattern; patch manually or use --force-main after review");
    }

    match replace_registration_block(&content) {
        Ok(patched) => Ok(patched),
        Err(_) => inject_tier1_registration(&content),
    }
}

/// Replace legacy `fallback_routes` split registration with Tier 1 gen + impl registry.
fn patch_fallback_registration(content: &str) -> anyhow::Result<String> {
    let start = content
        .find("let mut fallback_routes")
        .ok_or_else(|| anyhow::anyhow!("could not find fallback_routes registration block"))?;
    let end = find_fallback_registration_end(content, start)?;
    let indent = "    ";
    let replacement = format!(
        "{indent}// Register generated stubs, then override with impl controllers (ADR 0001 Tier 1)\n\
         {indent}unsafe {{\n\
         {indent}    gen_registry::register_from_spec(&mut dispatcher, &routes);\n\
         {indent}    impl_registry::register_impl(&mut dispatcher, &routes);\n\
         {indent}}}\n"
    );
    Ok(format!("{}{}{}", &content[..start], replacement, &content[end..]))
}

fn find_fallback_registration_end(content: &str, start: usize) -> anyhow::Result<usize> {
    let after_decl = content[start..]
        .find(';')
        .map(|i| start + i + 1)
        .ok_or_else(|| anyhow::anyhow!("fallback_routes declaration not terminated"))?;
    let rest = content[after_decl..].trim_start();
    let rest_start = after_decl + (content[after_decl..].len() - rest.len());

    if rest.starts_with("for route") {
        let for_idx = content[rest_start..]
            .find("for route")
            .map(|i| rest_start + i)
            .ok_or_else(|| anyhow::anyhow!("expected for route loop after fallback_routes"))?;
        let brace_open = content[for_idx..]
            .find('{')
            .map(|i| for_idx + i)
            .ok_or_else(|| anyhow::anyhow!("for route loop missing opening brace"))?;
        let for_close = find_closing_brace(content, brace_open)
            .ok_or_else(|| anyhow::anyhow!("unbalanced braces in for route loop"))?;
        let mut end = for_close + 1;
        let after_for = content[end..].trim_start();
        if after_for.starts_with("unsafe") {
            let unsafe_idx = end + (content[end..].len() - after_for.len());
            let brace_open = content[unsafe_idx..]
                .find('{')
                .map(|i| unsafe_idx + i)
                .ok_or_else(|| anyhow::anyhow!("unsafe block missing opening brace"))?;
            end = find_closing_brace(content, brace_open)
                .ok_or_else(|| anyhow::anyhow!("unbalanced braces in unsafe block"))?
                + 1;
        }
        if content[end..].starts_with('\n') {
            end += 1;
        }
        return Ok(end);
    }

    if rest.starts_with("unsafe") {
        let unsafe_idx = rest_start;
        let brace_open = content[unsafe_idx..]
            .find('{')
            .map(|i| unsafe_idx + i)
            .ok_or_else(|| anyhow::anyhow!("unsafe block missing opening brace"))?;
        let mut end = find_closing_brace(content, brace_open)
            .ok_or_else(|| anyhow::anyhow!("unbalanced braces in unsafe block"))?
            + 1;
        if content[end..].starts_with('\n') {
            end += 1;
        }
        return Ok(end);
    }

    anyhow::bail!("unrecognized fallback_routes registration pattern")
}

/// Services that register only via a manual `for route` loop (no `register_from_spec` yet).
fn inject_tier1_registration(content: &str) -> anyhow::Result<String> {
    let unsafe_idx = content
        .find("unsafe {")
        .ok_or_else(|| anyhow::anyhow!("could not find unsafe registration block in main.rs"))?;
    let block_open = unsafe_idx + "unsafe {".len();
    let block_close = find_closing_brace(content, unsafe_idx + "unsafe ".len())
        .ok_or_else(|| anyhow::anyhow!("unbalanced braces in unsafe block"))?;

    let block_body = &content[block_open..block_close];
    if !block_body.contains("for route in &routes")
        && !block_body.contains("for route in routes")
        && !block_body.contains("for route in routes.iter()")
    {
        anyhow::bail!("unsafe block has no for route loop to migrate");
    }

    let mut out = fix_gen_registry_import(content);
    out = ensure_controllers_mod(&out);
    if !out.contains("mod impl_registry") {
        out = insert_impl_registry_mod(&out);
    }

    let unsafe_idx = out
        .find("unsafe {")
        .ok_or_else(|| anyhow::anyhow!("could not find unsafe registration block in main.rs"))?;
    let block_open = unsafe_idx + "unsafe {".len();
    let indent = "        ";
    let injection = format!(
        "{indent}gen_registry::register_from_spec(&mut dispatcher, &routes);\n{indent}impl_registry::register_impl(&mut dispatcher, &routes);\n"
    );
    let injected = format!(
        "{}{}{}",
        &out[..block_open],
        injection,
        &out[block_open..]
    );
    remove_manual_match_registration_loop(&injected)
}

fn extract_gen_crate_root(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("use ") {
            continue;
        }
        let path = trimmed
            .trim_start_matches("use ")
            .trim_end_matches(';')
            .split(" as ")
            .next()?
            .trim();
        if path.ends_with("_gen") {
            return Some(path.to_string());
        }
        if let Some(idx) = path.find("_gen::") {
            return Some(path[..idx + 4].to_string());
        }
    }
    None
}

fn extract_gen_crate_from_registration(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(prefix) = line.split("::registry::register_from_spec").next() {
            let root = prefix.trim();
            if root.ends_with("_gen") {
                return Some(root.to_string());
            }
        }
    }
    extract_gen_crate_root(content)
}

fn fix_gen_registry_import(content: &str) -> String {
    let mut out = content.to_string();

    // Fix a prior bad patch: `handlers::registry` lives at crate root, not under handlers.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("::handlers::registry as gen_registry") {
            if let Some(root) = extract_gen_crate_root(content) {
                let fixed = format!("use {root}::registry as gen_registry;");
                out = out.replace(line, &line.replace(trimmed, &fixed));
                return out;
            }
        }
    }

    if content.contains(" as gen_registry") {
        return out;
    }

    if content.contains("gen_registry::") {
        if let Some(root) = extract_gen_crate_from_registration(content) {
            let alias_line = format!("use {root}::registry as gen_registry;");
            if !out.contains(&alias_line) {
                return insert_after_file_header(&out, &format!("{alias_line}\n"));
            }
        }
        return out;
    }

    // `use foo_gen::registry;` → alias
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("use ")
            && trimmed.ends_with("::registry;")
            && !trimmed.contains(" as ")
            && !trimmed.contains("::handlers::")
        {
            let replacement = trimmed.trim_end_matches(';').to_string() + " as gen_registry;";
            out = out.replace(line, &line.replace(trimmed, &replacement));
            return out;
        }
    }

    // `use foo_gen::*;` or `use foo_gen::handlers::*;` — insert explicit registry alias
    if let Some(root) = extract_gen_crate_root(content) {
        let alias_line = format!("use {root}::registry as gen_registry;");
        if !out.contains(&alias_line) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("use ") && trimmed.contains("_gen") && trimmed.contains("::*;") {
                    out = out.replace(line, &format!("{line}\n{alias_line}"));
                    return out;
                }
            }
        }
    }

    out
}

fn fix_misplaced_impl_registry_mod(content: &str) -> String {
    if !content.starts_with("mod impl_registry;\n") {
        return content.to_string();
    }
    let rest = &content["mod impl_registry;\n".len()..];
    if rest.contains("#![") {
        insert_after_file_header(rest, "mod impl_registry;\n")
    } else {
        content.to_string()
    }
}

fn insert_after_file_header(content: &str, insertion: &str) -> String {
    let mut insert_at = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        let line_len = line.len() + 1;
        if trimmed.starts_with("#![") || trimmed.starts_with("//") || trimmed.is_empty() {
            insert_at += line_len;
            continue;
        }
        break;
    }
    format!("{}{}{}", &content[..insert_at], insertion, &content[insert_at..])
}

fn remove_manual_match_registration_loop(content: &str) -> anyhow::Result<String> {
    let Some(for_idx) = content
        .find("for route in &routes")
        .or_else(|| content.find("for route in routes"))
        .or_else(|| content.find("for route in routes.iter()"))
    else {
        return Ok(content.to_string());
    };

    let mut remove_start = content[..for_idx].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let before = content[remove_start..for_idx].trim();
    if before.starts_with("//") {
        remove_start = content[..remove_start]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
    }

    let brace_open = content[for_idx..]
        .find('{')
        .map(|i| for_idx + i)
        .ok_or_else(|| anyhow::anyhow!("for route loop missing opening brace"))?;
    let brace_close = find_closing_brace(content, brace_open)
        .ok_or_else(|| anyhow::anyhow!("unbalanced braces in for route loop"))?;

    let mut end = brace_close + 1;
    if content[end..].starts_with('\n') {
        end += 1;
    }

    Ok(format!("{}{}", &content[..remove_start], &content[end..]))
}

fn rewrite_register_from_spec_line(line: &str) -> String {
    if line.contains("gen_registry::register_from_spec") {
        return line.to_string();
    }
    if let Some(prefix) = line.split("::registry::register_from_spec").next() {
        if prefix.trim().ends_with("_gen") {
            let qualified = format!("{}::registry::register_from_spec", prefix.trim());
            return line.replace(&qualified, "gen_registry::register_from_spec");
        }
    }
    line.replace("registry::register_from_spec", "gen_registry::register_from_spec")
}

fn fix_registration_lines(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.contains("register_from_spec") {
                rewrite_register_from_spec_line(line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn infer_gen_crate_from_impl_dir(impl_output_dir: &Path) -> Option<String> {
    let cargo = fs::read_to_string(impl_output_dir.join("Cargo.toml")).ok()?;
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name = ") {
            let name = trimmed
                .trim_start_matches("name = ")
                .trim()
                .trim_matches('"');
            return Some(format!("{name}_gen"));
        }
    }
    None
}

fn ensure_gen_registry_import_for_impl(content: &str, impl_output_dir: &Path) -> String {
    if !content.contains("gen_registry::") || content.contains(" as gen_registry") {
        return content.to_string();
    }
    if let Some(gen) = infer_gen_crate_from_impl_dir(impl_output_dir) {
        let alias_line = format!("use {gen}::registry as gen_registry;");
        if !content.contains(&alias_line) {
            return insert_after_file_header(content, &format!("{alias_line}\n"));
        }
    }
    content.to_string()
}

fn package_uses_lib_controllers(impl_output_dir: &Path) -> bool {
    let lib_path = impl_output_dir.join("src/lib.rs");
    lib_path.is_file()
        && fs::read_to_string(lib_path)
            .map(|c| c.contains("pub mod controllers"))
            .unwrap_or(false)
}

fn ensure_controllers_mod(content: &str) -> String {
    if content.contains("mod controllers") {
        return content.to_string();
    }
    insert_after_file_header(content, "mod controllers;\n")
}

fn ensure_controllers_mod_for_impl(content: &str, impl_output_dir: &Path) -> String {
    if package_uses_lib_controllers(impl_output_dir) {
        return content.to_string();
    }
    ensure_controllers_mod(content)
}

fn insert_impl_registry_mod(content: &str) -> String {
    let content = ensure_controllers_mod(content);
    if content.contains("mod impl_registry") {
        return content.to_string();
    }

    // After `mod controllers;`
    if let Some(idx) = content.find("mod controllers;") {
        let insert_at = idx + "mod controllers;".len();
        return format!(
            "{}\nmod impl_registry;{}",
            &content[..insert_at],
            &content[insert_at..]
        );
    }

    // After inline `mod controllers { ... }` block
    if let Some(start) = content.find("mod controllers") {
        if let Some(rel_open) = content[start..].find('{') {
            let open = start + rel_open;
            if let Some(close) = find_closing_brace(&content, open) {
                return format!(
                    "{}\nmod impl_registry;{}",
                    &content[..=close],
                    &content[close + 1..]
                );
            }
        }
    }

    insert_after_file_header(&content, "mod impl_registry;\n")
}

fn replace_registration_block(content: &str) -> anyhow::Result<String> {
    let reg_needle = "register_from_spec(&mut dispatcher";
    let reg_idx = content
        .find(reg_needle)
        .ok_or_else(|| anyhow::anyhow!("could not find register_from_spec in main.rs"))?;

    let line_start = content[..reg_idx].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = content[reg_idx..]
        .find('\n')
        .map(|i| reg_idx + i + 1)
        .unwrap_or(content.len());

    let reg_line = &content[line_start..line_end.min(content.len())];
    let new_reg_line = rewrite_register_from_spec_line(reg_line);

    let mut tail_start = line_end;
    let rest = content[tail_start..].trim_start();
    let ws = content[tail_start..].len() - rest.len();
    tail_start += ws;

    let indent = "        ";
    let replacement_tail = if rest.starts_with("for route in &routes")
        || rest.starts_with("for route in routes")
    {
        let brace_open = tail_start + rest.find('{').unwrap();
        let brace_close = find_closing_brace(content, brace_open)
            .ok_or_else(|| anyhow::anyhow!("unbalanced braces in for route loop"))?;
        format!(
            "{new_reg_line}{indent}impl_registry::register_impl(&mut dispatcher, &routes);\n{}",
            &content[brace_close + 1..]
        )
    } else {
        format!(
            "{new_reg_line}{indent}impl_registry::register_impl(&mut dispatcher, &routes);\n{}",
            &content[tail_start..]
        )
    };

    let patched = format!("{}{}", &content[..line_start], replacement_tail);
    remove_manual_match_registration_loop(&patched)
}

pub fn migrate_registration(opts: &MigrateRegistrationOptions) -> anyhow::Result<MigrationReport> {
    let spec_str = opts
        .spec_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in spec path"))?;
    let (routes, _slug) = crate::spec::load_spec(spec_str)?;

    let impl_src_dir = opts.impl_output_dir.join("src");
    let main_path = impl_src_dir.join("main.rs");
    let main_content = if main_path.is_file() {
        fs::read_to_string(&main_path)?
    } else {
        String::new()
    };

    let report = analyze_migration(&routes, &impl_src_dir, &main_content)?;
    print_migration_report(&report, &impl_src_dir);

    if !opts.apply {
        println!("\n(dry-run — pass --apply to write impl_registry.rs and patch main.rs)");
        return Ok(report);
    }

    if !report.plan.errors.is_empty() {
        anyhow::bail!(
            "cannot apply: fix {} plan error(s) first (see report above)",
            report.plan.errors.len()
        );
    }

    let mut scoped_handlers = extract_main_match_handlers(&main_content);
    if scoped_handlers.is_empty() {
        let impl_reg_path = impl_src_dir.join("impl_registry.rs");
        if impl_reg_path.is_file() {
            let reg_content = fs::read_to_string(&impl_reg_path)?;
            scoped_handlers = extract_registry_file_handlers(&reg_content);
        }
    }
    if scoped_handlers.is_empty() {
        let legacy_reg_path = impl_src_dir.join("registry.rs");
        if legacy_reg_path.is_file() {
            let reg_content = fs::read_to_string(&legacy_reg_path)?;
            scoped_handlers = extract_registry_file_handlers(&reg_content);
        }
    }

    let plan = if scoped_handlers.is_empty() {
        crate::generator::plan_impl_registry(&routes, &impl_src_dir.join("controllers"))?
    } else {
        crate::generator::plan_impl_registry_for_migration(
            &routes,
            &impl_src_dir.join("controllers"),
            &scoped_handlers,
        )?
    };

    // Partial wiring: only regen mod.rs when every disk controller is in scope.
    let regen_mod = !main_content.contains("mod controllers {")
        && scoped_handlers.is_empty()
        && !package_uses_lib_controllers(&opts.impl_output_dir);
    crate::generator::write_impl_registry_from_plan(&impl_src_dir, &plan, regen_mod)?;

    let needs_main_patch = !report.already_migrated
        || main_content.contains("match route.handler_name.as_ref()")
        || main_content.contains("match handler_name.as_ref()")
        || main_content.contains("fallback_routes")
        || main_content.contains("for route in routes.iter()")
        || main_content.contains("::handlers::registry as gen_registry")
        || main_content.starts_with("mod impl_registry;\n//");

    if report.already_migrated && !needs_main_patch {
        println!("ℹ️  main.rs already calls impl_registry::register_impl — skipped main patch");
        return Ok(report);
    }

    if report.complex_main
        && !opts.force_main
        && !main_content.contains("fallback_routes")
    {
        anyhow::bail!(
            "refusing to patch complex main.rs: {}. Use --force-main after manual review, or edit main.rs by hand.",
            report.complex_main_reason.as_deref().unwrap_or("unknown")
        );
    }

    if main_path.is_file() {
        if report.complex_main && opts.force_main {
            println!("⚠️  --force-main: attempting patch on complex main (verify cargo check!)");
        }
        let mut fixed = patch_main_registration_simple(&main_content, opts.force_main)?;
        if package_uses_lib_controllers(&opts.impl_output_dir) {
            fixed = fixed.replace("mod controllers;\n", "");
            fixed = fix_registration_lines(&fixed);
        }
        fixed = fix_gen_registry_import(&fixed);
        fixed = ensure_gen_registry_import_for_impl(&fixed, &opts.impl_output_dir);
        fs::write(&main_path, fixed)?;
        println!("✅ Patched main.rs → {main_path:?}");
    } else {
        println!("⚠️  no main.rs at {main_path:?}; wrote impl_registry only");
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_MAIN: &str = r#"
use sesame_idam_org_mgmt_gen::registry;
mod controllers;

fn main() {
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
        for route in &routes {
            match route.handler_name.as_ref() {
                "create_organization" => {}
                "list_my_memberships" => {}
                _ => {}
            }
        }
    }
}
"#;

    #[test]
    fn extracts_match_arm_handlers() {
        let h = extract_main_match_handlers(SIMPLE_MAIN);
        assert!(h.contains("create_organization"));
        assert!(h.contains("list_my_memberships"));
        assert!(!h.contains("_"));
    }

    #[test]
    fn patches_simple_main() {
        let patched = patch_main_registration_simple(SIMPLE_MAIN, false).unwrap();
        assert!(patched.contains("gen_registry::register_from_spec"));
        assert!(patched.contains("impl_registry::register_impl"));
        assert!(patched.contains("mod impl_registry;"));
        assert!(!patched.contains("match route.handler_name"));
    }

    #[test]
    fn extracts_if_route_handler_checks() {
        let main = r#"
unsafe {
    registry::register_from_spec(&mut dispatcher, &routes);
    for route in &routes {
        if route.handler_name.as_ref() == "login_user" { }
        else if route.handler_name.as_ref() == "create_organization" { }
    }
}
"#;
        let h = extract_main_match_handlers(main);
        assert!(h.contains("login_user"));
        assert!(h.contains("create_organization"));
    }

    #[test]
    fn ignores_auth_match_false_positives() {
        let main = r#"
unsafe {
    hauliage_terminals_gen::registry::register_from_spec(&mut dispatcher, &routes);
}
match source.as_str() {
    "header" => {}
    "query" => {}
    "cookie" => {}
    _ => {}
}
"#;
        let h = extract_main_match_handlers(main);
        assert!(h.is_empty());
    }

    #[test]
    fn patches_fallback_split_registration() {
        let main = r#"
use foo_gen::registry;
mod controllers;

fn main() {
    let mut fallback_routes = Vec::new();
    for route in &routes {
        match route.handler_name.as_ref() {
            "generate_document_upload_url" => {}
            _ => { fallback_routes.push(route.clone()); }
        }
    }
    unsafe {
        registry::register_from_spec(&mut dispatcher, &fallback_routes);
    }
}
"#;
        let patched = patch_main_registration_simple(main, false).unwrap();
        assert!(patched.contains("gen_registry::register_from_spec(&mut dispatcher, &routes)"));
        assert!(patched.contains("impl_registry::register_impl"));
        assert!(!patched.contains("fallback_routes"));
    }

    #[test]
    fn extracts_handler_name_match_in_iter_loop() {
        let main = r#"
unsafe {
    for route in routes.iter() {
        match handler_name.as_ref() {
            "submit_quote" => {}
            "accept_quote" => {}
            _ => {}
        }
    }
}
"#;
        let h = extract_main_match_handlers(main);
        assert!(h.contains("submit_quote"));
        assert!(h.contains("accept_quote"));
    }

    #[test]
    fn refuses_complex_company_pattern() {
        let complex = r#"
for route in &routes { match route.handler_name.as_ref() { "a" => {}, _ => {} } }
for route in &routes { match route.handler_name.as_ref() { "b" => {}, _ => {} } }
"#;
        assert!(detect_complex_main(complex).is_some());
        assert!(patch_main_registration_simple(complex, false).is_err());
    }

    #[test]
    fn inserts_impl_registry_mod_after_inline_controllers() {
        let inline = r#"
mod controllers {
    pub mod create_organization;
}
use foo_gen::registry;
"#;
        let out = insert_impl_registry_mod(inline);
        assert!(out.contains("}\nmod impl_registry;"));
    }

    #[test]
    fn fixes_handlers_glob_gen_registry_import() {
        let main = r#"
use hauliage_analytics_gen::handlers::*;
use hauliage_analytics_gen::*;

fn main() {
    unsafe {
        gen_registry::register_from_spec(&mut dispatcher, &routes);
        impl_registry::register_impl(&mut dispatcher, &routes);
        for route in &routes {
            match route.handler_name.as_ref() {
                "get_fleet_performance" => {}
                _ => {}
            }
        }
    }
}
"#;
        let main = main.replace(
            "gen_registry::register_from_spec",
            "registry::register_from_spec",
        );
        let patched = patch_main_registration_simple(&main, false).unwrap();
        assert!(patched.contains("use hauliage_analytics_gen::registry as gen_registry;"));
        assert!(!patched.contains("handlers::registry"));
        assert!(!patched.contains("match route.handler_name"));
    }

    #[test]
    fn inserts_impl_registry_mod_after_inner_attr() {
        let inline = r#"// header
#![allow(clippy::uninlined_format_args)]
use hauliage_consignments_gen::*;
use hauliage_consignments::controllers;
"#;
        let out = insert_impl_registry_mod(inline);
        assert!(out.contains("#![allow(clippy::uninlined_format_args)]\nmod impl_registry;\nuse "));
    }
}
