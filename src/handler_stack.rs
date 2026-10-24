//! The stack size of each handler coroutine, resolved at runtime from the OpenAPI spec with environment overrides.
//!
//! The spec is the place to size handlers; environment variables override it without a rebuild. The most specific
//! setting wins, and at each level the environment overrides the spec:
//!
//! 1. `BRRTR_STACK_SIZE__<HANDLER>` (env, this handler; name upper-cased)
//! 2. `x-brrtrouter-stack-size` on the operation (spec, this handler)
//! 3. `BRRTR_STACK_SIZE` (env, every handler)
//! 4. `x-brrtrouter-stack-size` at the root of the spec (spec, every handler)
//! 5. the size the generator computed for the route (passed in by the generated registry)
//!
//! The result is clamped to `BRRTR_STACK_MIN_BYTES` ..= `BRRTR_STACK_MAX_BYTES` (16 KiB ..= 256 KiB by default).
//!
//! Spec values are recorded whenever routes are built from a spec ([`record_spec`], called by
//! `spec::build_routes_with_security_presence`), so the running service follows the spec it loaded: changing a
//! handler's size needs no code generation. Values accept bytes (`65536`, `"0x10000"`), units (`"64KiB"`, `"64K"`,
//! `"1MiB"`) or `"small"` (16 KiB), `"medium"` (32 KiB), `"large"` (64 KiB).
//!
//! Note: `may` takes the size in machine words, so a coroutine maps 8x the resolved value (see
//! `llmwiki/topics/handler-stack-memory.md`); `brrtrouter_handler_stack_size_bytes` shows what is really mapped.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::spec::RouteMeta;

#[derive(Debug, Default)]
struct SpecSizes {
    default: Option<usize>,
    per_handler: HashMap<String, usize>,
}

fn spec_sizes() -> &'static RwLock<SpecSizes> {
    static SIZES: OnceLock<RwLock<SpecSizes>> = OnceLock::new();
    SIZES.get_or_init(|| RwLock::new(SpecSizes::default()))
}

/// Record the stack sizes a spec declares: the root-level default and each operation's
/// (`RouteMeta::x_brrtrouter_stack_size`). Replaces what an earlier spec recorded (hot reload).
pub fn record_spec(spec_default: Option<usize>, routes: &[RouteMeta]) {
    let per_handler = routes
        .iter()
        .filter_map(|r| {
            r.x_brrtrouter_stack_size
                .map(|s| (r.handler_name.to_string(), s))
        })
        .collect();
    let mut sizes = spec_sizes()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *sizes = SpecSizes {
        default: spec_default,
        per_handler,
    };
}

/// Parse a stack size: decimal or `0x` hex bytes, a `K`/`KiB`/`M`/`MiB` suffix (binary units), or
/// `small` / `medium` / `large`.
pub fn parse_size(s: &str) -> Option<usize> {
    let s = s.trim();
    match s.to_ascii_lowercase().as_str() {
        "small" => return Some(16 * 1024),
        "medium" => return Some(32 * 1024),
        "large" => return Some(64 * 1024),
        _ => {}
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return usize::from_str_radix(hex, 16).ok();
    }
    let lower = s.to_ascii_lowercase();
    for (suffix, mult) in [
        ("kib", 1024usize),
        ("k", 1024),
        ("mib", 1024 * 1024),
        ("m", 1024 * 1024),
    ] {
        if let Some(n) = lower.strip_suffix(suffix) {
            return n.trim().parse::<usize>().ok()?.checked_mul(mult);
        }
    }
    s.parse().ok()
}

/// A stack size from a spec extension value (number of bytes or a string [`parse_size`] accepts).
pub fn size_from_value(v: &serde_json::Value) -> Option<usize> {
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .or_else(|| v.as_str().and_then(parse_size))
}

fn env_size(key: &str) -> Option<usize> {
    std::env::var(key).ok().and_then(|v| parse_size(&v))
}

/// The stack size (bytes) for `handler`, `generated` being the generator's computed size. See the module docs for
/// the precedence.
pub fn resolve(handler: &str, generated: usize) -> usize {
    let (spec_handler, spec_default) = {
        let sizes = spec_sizes()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (sizes.per_handler.get(handler).copied(), sizes.default)
    };
    let size = env_size(&format!("BRRTR_STACK_SIZE__{}", handler.to_uppercase()))
        .or(spec_handler)
        .or_else(|| env_size("BRRTR_STACK_SIZE"))
        .or(spec_default)
        .unwrap_or(generated);
    let min = env_size("BRRTR_STACK_MIN_BYTES").unwrap_or(16 * 1024);
    let max = env_size("BRRTR_STACK_MAX_BYTES").unwrap_or(256 * 1024);
    size.clamp(min, max.max(min))
}

/// Serializes tests that set the stack-size environment variables (process-global) across modules.
#[cfg(test)]
pub(crate) static STACK_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("65536"), Some(65536));
        assert_eq!(parse_size("0x10000"), Some(65536));
        assert_eq!(parse_size("64KiB"), Some(65536));
        assert_eq!(parse_size("64k"), Some(65536));
        assert_eq!(parse_size("1MiB"), Some(1 << 20));
        assert_eq!(parse_size("large"), Some(65536));
        assert_eq!(parse_size("lots"), None);
        assert_eq!(size_from_value(&serde_json::json!(32768)), Some(32768));
        assert_eq!(size_from_value(&serde_json::json!("48K")), Some(48 * 1024));
    }

    /// One test (the registry and env are process-global): a real spec parsed with oas3 (which strips the
    /// `x-` prefix from extension keys), then the precedence end to end.
    #[test]
    fn precedence_most_specific_first_env_over_spec() {
        let _env = STACK_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for k in [
            "BRRTR_STACK_SIZE",
            "BRRTR_STACK_MIN_BYTES",
            "BRRTR_STACK_MAX_BYTES",
        ] {
            // SAFETY (test): env access serialized by STACK_ENV_LOCK.
            unsafe { std::env::remove_var(k) };
        }
        let yaml = r#"
openapi: 3.1.0
info: { title: t, version: "1" }
x-brrtrouter-stack-size: 24KiB
paths:
  /deep:
    get:
      operationId: probe_spec_handler
      x-brrtrouter-stack-size: 48KiB
      responses: { "200": { description: ok } }
  /plain:
    get:
      operationId: probe_plain_handler
      responses: { "200": { description: ok } }
"#;
        let spec = oas3::from_yaml(yaml).expect("spec parses");
        let routes = crate::spec::build_routes(&spec, "t").expect("routes");
        let deep = routes
            .iter()
            .find(|r| &*r.handler_name == "probe_spec_handler")
            .expect("route");
        assert_eq!(deep.x_brrtrouter_stack_size, Some(48 * 1024));

        // spec per-operation beats spec default; spec default beats generated
        assert_eq!(resolve("probe_spec_handler", 20 * 1024), 48 * 1024);
        assert_eq!(resolve("probe_plain_handler", 20 * 1024), 24 * 1024);

        // per-handler env beats the operation's spec value; global env beats the spec default only
        unsafe { std::env::set_var("BRRTR_STACK_SIZE__PROBE_SPEC_HANDLER", "100K") };
        unsafe { std::env::set_var("BRRTR_STACK_SIZE", "0x10000") };
        assert_eq!(resolve("probe_spec_handler", 20 * 1024), 100 * 1024);
        unsafe { std::env::remove_var("BRRTR_STACK_SIZE__PROBE_SPEC_HANDLER") };
        assert_eq!(resolve("probe_spec_handler", 20 * 1024), 48 * 1024);
        assert_eq!(resolve("probe_plain_handler", 20 * 1024), 64 * 1024);
        unsafe { std::env::remove_var("BRRTR_STACK_SIZE") };

        // clamped
        record_spec(None, &[]);
        assert_eq!(resolve("probe_plain_handler", 1024), 16 * 1024);
        assert_eq!(resolve("probe_plain_handler", 10 << 20), 256 * 1024);
    }
}
