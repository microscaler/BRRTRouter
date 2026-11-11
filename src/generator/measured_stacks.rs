//! `brrtrouter-gen stack-sizes`: each handler's `x-brrtrouter-stack-size` from measured peak stack use.
//!
//! Input is one service's measurements, `[{"handler": "...", "peak_bytes": N, "samples": N}, ...]`, for example
//! from Prometheus: `max_over_time(brrtrouter_handler_stack_peak_bytes[14d])` for the peak and
//! `increase(brrtrouter_handler_stack_used_bytes_count[14d])` for the samples (14 days: older peaks may predate a
//! probe fix). The command needs no metrics credentials; it reads the file.
//!
//! For each operation of the spec (its `operationId` is the handler name):
//!
//! - `suggested = clamp(round_up(peak × k, round), floor, max)`, `k` being `--safety`, or `--low-sample-safety`
//!   for a handler with fewer than `--min-samples` samples;
//! - the size changes only when `|suggested − current| > hysteresis × current`, or when `current / peak < 1.5`;
//! - a handler below `--min-samples` is never lowered, and an operation without measurements is left alone;
//! - a suggestion at `--max` (default `BRRTR_STACK_MAX_BYTES`, else 256 KiB) is flagged: the runtime clamps to that
//!   maximum, so it must be raised first.
//!
//! `current` is what the runtime resolves from the spec: the operation's value, else the spec root's, else the
//! 256 KiB fallback, clamped to `--max`. Without `--write` the command prints a table and a unified diff; with it, it
//! sets `x-brrtrouter-stack-size: "<n>KiB"` on each changed operation, editing the YAML text in place so comments
//! and layout are kept. `--target suite-config` writes into the sub-specs a BFF spec was merged from (the
//! operation's `x-service`, mapped through the BFF suite config) instead of the generated BFF spec.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::handler_stack::{parse_size, size_from_value, FALLBACK};

/// One handler's measurements.
#[derive(Debug, Clone, Deserialize)]
pub struct Measurement {
    /// Handler name (the operation's `operationId`).
    pub handler: String,
    /// Peak stack use seen, bytes.
    pub peak_bytes: u64,
    /// How many requests were measured.
    pub samples: u64,
}

/// The sizing rules (see the module docs).
#[derive(Debug, Clone)]
pub struct Rules {
    /// Factor over the peak.
    pub safety: f64,
    /// Factor over the peak below `min_samples`.
    pub low_sample_safety: f64,
    /// Samples needed to trust a peak (and to lower a size).
    pub min_samples: u64,
    /// Smallest size suggested, bytes.
    pub floor: usize,
    /// Sizes are rounded up to a multiple of this, bytes.
    pub round: usize,
    /// Largest size (the runtime's `BRRTR_STACK_MAX_BYTES`), bytes.
    pub max: usize,
    /// Relative change below which a size is kept.
    pub hysteresis: f64,
}

/// Where `--write` puts the sizes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum StackSizeTarget {
    /// The spec given with `--spec`.
    Spec,
    /// The sub-specs a BFF spec was merged from (`x-service` → the BFF suite config's `spec_path`).
    SuiteConfig,
}

/// What to do with one handler's size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Leave it, and why.
    Keep(&'static str),
    /// Set it to this many bytes.
    Set(usize),
}

/// One operation's line in the report.
#[derive(Debug, Clone)]
pub struct Row {
    /// Handler name.
    pub handler: String,
    /// Size the runtime resolves today, bytes.
    pub current: usize,
    /// The handler's measurements, if any.
    pub measured: Option<(u64, u64)>,
    /// What the rules suggest, bytes.
    pub suggested: Option<usize>,
    /// The decision.
    pub action: Action,
    /// The suggestion is at `max`: raise `BRRTR_STACK_MAX_BYTES` before relying on it.
    pub at_max: bool,
}

/// The suggestion for one handler: `clamp(round_up(peak × k, round), floor, max)`.
#[must_use]
pub fn suggest(m: &Measurement, r: &Rules) -> usize {
    let k = if m.samples < r.min_samples {
        r.low_sample_safety
    } else {
        r.safety
    };
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let wanted = (m.peak_bytes as f64 * k).ceil() as usize;
    let round = r.round.max(1);
    let rounded = wanted.div_ceil(round).saturating_mul(round);
    rounded.clamp(r.floor.min(r.max), r.max)
}

/// The decision for one handler whose size resolves to `current` today.
#[must_use]
pub fn decide(current: usize, m: Option<&Measurement>, r: &Rules) -> (Option<usize>, Action) {
    let Some(m) = m else {
        return (None, Action::Keep("no measurements"));
    };
    let suggested = suggest(m, r);
    if suggested == current {
        return (Some(suggested), Action::Keep("unchanged"));
    }
    if m.samples < r.min_samples && suggested < current {
        return (Some(suggested), Action::Keep("too few samples to lower"));
    }
    #[allow(clippy::cast_precision_loss)]
    let (diff, cur, peak) = (
        suggested.abs_diff(current) as f64,
        current as f64,
        m.peak_bytes as f64,
    );
    let tight = m.peak_bytes > 0 && cur / peak < 1.5;
    if diff > r.hysteresis * cur || tight {
        (Some(suggested), Action::Set(suggested))
    } else {
        (Some(suggested), Action::Keep("within hysteresis"))
    }
}

const KEY: &str = "x-brrtrouter-stack-size";
const METHODS: [&str; 10] = [
    "get",
    "put",
    "post",
    "delete",
    "options",
    "head",
    "patch",
    "trace",
    "query",
    "x-brrtrouter-query",
];

/// An operation of the spec: its handler name, its own size, and the BFF sub-service it came from.
struct Operation {
    handler: String,
    own: Option<usize>,
    service: Option<String>,
}

fn json(v: &serde_yaml::Value) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

/// The spec's operations and its root size.
fn operations(spec: &serde_yaml::Value) -> (Vec<Operation>, Option<usize>) {
    let root = spec.get(KEY).and_then(|v| size_from_value(&json(v)));
    let mut ops = Vec::new();
    if let Some(paths) = spec.get("paths").and_then(serde_yaml::Value::as_mapping) {
        for item in paths.values() {
            for method in METHODS {
                let Some(op) = item.get(method) else { continue };
                let Some(id) = op.get("operationId").and_then(serde_yaml::Value::as_str) else {
                    continue;
                };
                ops.push(Operation {
                    handler: id.to_string(),
                    own: op.get(KEY).and_then(|v| size_from_value(&json(v))),
                    service: op
                        .get("x-service")
                        .and_then(serde_yaml::Value::as_str)
                        .map(str::to_string),
                });
            }
        }
    }
    (ops, root)
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Set `x-brrtrouter-stack-size: <value>` on the operation whose `operationId` is `op_id`, editing the YAML text so
/// comments and layout are kept: the key is replaced when the operation has it, else added after `operationId`.
/// `None` when no block-style `operationId: <op_id>` line is found.
#[must_use]
pub fn set_in_text(text: &str, op_id: &str, value: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|l| {
        l.trim_start()
            .strip_prefix("operationId:")
            .map(|v| v.trim().trim_matches(|c| c == '"' || c == '\''))
            == Some(op_id)
    })?;
    let ind = indent(lines[at]);
    let in_block = |l: &str| l.trim().is_empty() || indent(l) >= ind;
    let start = at - lines[..at].iter().rev().take_while(|l| in_block(l)).count();
    let end = at + 1 + lines[at + 1..].iter().take_while(|l| in_block(l)).count();
    let pad = " ".repeat(ind);
    let new_line = format!("{pad}{KEY}: \"{value}\"");
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_string()).collect();
    match (start..end).find(|&i| indent(lines[i]) == ind && lines[i].trim_start().starts_with(KEY))
    {
        Some(i) => out[i] = new_line,
        None => out.insert(at + 1, new_line),
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    Some(s)
}

/// A unified diff of `old` → `new` for edits that replace or insert single lines (what [`set_in_text`] does).
#[must_use]
pub fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    // (old index, new index, removed, added)
    let mut changes: Vec<(usize, usize, Option<&str>, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while j < b.len() {
        if i < a.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if i < a.len() && b.get(j + 1) == Some(&a[i]) {
            changes.push((i, j, None, b[j]));
            j += 1;
        } else {
            changes.push((i, j, a.get(i).copied(), b[j]));
            i += 1;
            j += 1;
        }
    }
    if changes.is_empty() {
        return String::new();
    }
    let mut out = format!("--- {0}\n+++ {0}\n", path.display());
    for (i, j, removed, added) in changes {
        let ctx = 2;
        let from = i.saturating_sub(ctx);
        let before = &a[from..i];
        let after_start = i + usize::from(removed.is_some());
        let after = &a[after_start.min(a.len())..(after_start + ctx).min(a.len())];
        let old_len = before.len() + usize::from(removed.is_some()) + after.len();
        let new_len = before.len() + 1 + after.len();
        let _ = writeln!(
            out,
            "@@ -{},{} +{},{} @@",
            from + 1,
            old_len,
            j - before.len() + 1,
            new_len
        );
        for l in before {
            let _ = writeln!(out, " {l}");
        }
        if let Some(r) = removed {
            let _ = writeln!(out, "-{r}");
        }
        let _ = writeln!(out, "+{added}");
        for l in after {
            let _ = writeln!(out, " {l}");
        }
    }
    out
}

/// The BFF suite config's sub-spec for each service: `openapi_base_dir` (relative to `base_dir`) joined with each
/// `services.<name>.spec_path` (default `<name>/openapi.yaml`).
fn suite_specs(config: &Path, base_dir: &Path) -> anyhow::Result<BTreeMap<String, PathBuf>> {
    let data: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(config)?)?;
    let openapi_base = base_dir.join(
        data.get("openapi_base_dir")
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or(""),
    );
    let mut out = BTreeMap::new();
    if let Some(services) = data.get("services").and_then(serde_yaml::Value::as_mapping) {
        for (name, svc) in services {
            let Some(name) = name.as_str() else { continue };
            let spec_path = svc
                .get("spec_path")
                .and_then(serde_yaml::Value::as_str)
                .map_or_else(|| format!("{name}/openapi.yaml"), str::to_string);
            out.insert(name.to_string(), openapi_base.join(spec_path));
        }
    }
    Ok(out)
}

/// The report: one row per operation, the edited files, and warnings.
#[derive(Debug, Default)]
pub struct Report {
    /// One line per operation.
    pub rows: Vec<Row>,
    /// File → (old text, new text), for each file with changes.
    pub edits: BTreeMap<PathBuf, (String, String)>,
    /// Things to act on before relying on the result.
    pub warnings: Vec<String>,
}

/// Size in KiB for the spec (sizes are multiples of `round`, itself normally a multiple of 1 KiB).
fn kib(bytes: usize) -> String {
    if bytes.is_multiple_of(1024) {
        format!("{}KiB", bytes / 1024)
    } else {
        bytes.to_string()
    }
}

/// Plan the sizes for `spec` from `measurements` (see the module docs).
///
/// # Errors
///
/// The files cannot be read or parsed, or an operation to change cannot be found in its file's text.
pub fn plan(
    measurements: &[Measurement],
    spec: &Path,
    rules: &Rules,
    target: StackSizeTarget,
    suite: Option<(&Path, &Path)>,
) -> anyhow::Result<Report> {
    let spec_value: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(spec)?)?;
    let (ops, root) = operations(&spec_value);
    let by_handler: BTreeMap<&str, &Measurement> = measurements
        .iter()
        .map(|m| (m.handler.as_str(), m))
        .collect();
    let sources = match (target, suite) {
        (StackSizeTarget::Spec, _) => BTreeMap::new(),
        (StackSizeTarget::SuiteConfig, Some((config, base))) => suite_specs(config, base)?,
        (StackSizeTarget::SuiteConfig, None) => {
            anyhow::bail!("--target suite-config needs --suite-config")
        }
    };
    let mut report = Report::default();
    for op in &ops {
        let current = op.own.or(root).unwrap_or(FALLBACK).min(rules.max);
        let m = by_handler.get(op.handler.as_str()).copied();
        let (suggested, action) = decide(current, m, rules);
        let at_max = suggested == Some(rules.max);
        if at_max {
            report.warnings.push(format!(
                "{}: the suggestion is the maximum ({}); raise BRRTR_STACK_MAX_BYTES before relying on it",
                op.handler,
                kib(rules.max)
            ));
        }
        if let Action::Set(bytes) = action {
            let file = match target {
                StackSizeTarget::Spec => spec.to_path_buf(),
                StackSizeTarget::SuiteConfig => {
                    let Some(file) = op.service.as_ref().and_then(|s| sources.get(s)) else {
                        report.warnings.push(format!(
                            "{}: no sub-spec for it in the suite config (x-service {:?}); not written",
                            op.handler, op.service
                        ));
                        continue;
                    };
                    file.clone()
                }
            };
            let entry = match report.edits.entry(file.clone()) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => {
                    let text = std::fs::read_to_string(&file)?;
                    e.insert((text.clone(), text))
                }
            };
            entry.1 = set_in_text(&entry.1, &op.handler, &kib(bytes)).ok_or_else(|| {
                anyhow::anyhow!(
                    "{}: no block-style `operationId: {}` in {}",
                    op.handler,
                    op.handler,
                    file.display()
                )
            })?;
        }
        report.rows.push(Row {
            handler: op.handler.clone(),
            current,
            measured: m.map(|m| (m.peak_bytes, m.samples)),
            suggested,
            action,
            at_max,
        });
    }
    for m in measurements {
        if !ops.iter().any(|o| o.handler == m.handler) {
            report.warnings.push(format!(
                "{}: measured, but no such operation in the spec",
                m.handler
            ));
        }
    }
    Ok(report)
}

impl Report {
    /// The table, the warnings and a unified diff per changed file.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<40} {:>9} {:>9} {:>9} {:>10}  action",
            "handler", "current", "peak", "samples", "suggested"
        );
        for r in &self.rows {
            let (peak, samples) = r
                .measured
                .map_or(("-".to_string(), "-".to_string()), |(p, s)| {
                    (p.to_string(), s.to_string())
                });
            let action = match &r.action {
                Action::Keep(why) => format!("keep ({why})"),
                Action::Set(b) => format!("set {}", kib(*b)),
            };
            let _ = writeln!(
                out,
                "{:<40} {:>9} {:>9} {:>9} {:>10}  {action}{}",
                r.handler,
                kib(r.current),
                peak,
                samples,
                r.suggested.map_or_else(|| "-".to_string(), kib),
                if r.at_max { "  [at max]" } else { "" }
            );
        }
        for w in &self.warnings {
            let _ = writeln!(out, "warning: {w}");
        }
        for (path, (old, new)) in &self.edits {
            out.push_str(&unified_diff(path, old, new));
        }
        out
    }

    /// Write the edited files.
    ///
    /// # Errors
    ///
    /// A file cannot be written.
    pub fn write(&self) -> anyhow::Result<()> {
        for (path, (_, new)) in &self.edits {
            std::fs::write(path, new)?;
        }
        Ok(())
    }
}

/// A size option (`64KiB`, `65536`, ...) in bytes.
///
/// # Errors
///
/// Not a size [`parse_size`] accepts.
pub fn size_arg(name: &str, value: &str) -> anyhow::Result<usize> {
    parse_size(value).ok_or_else(|| anyhow::anyhow!("--{name}: not a size: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules {
            safety: 1.5,
            low_sample_safety: 2.0,
            min_samples: 1000,
            floor: 64 * 1024,
            round: 16 * 1024,
            max: 256 * 1024,
            hysteresis: 0.25,
        }
    }

    fn m(handler: &str, peak: u64, samples: u64) -> Measurement {
        Measurement {
            handler: handler.into(),
            peak_bytes: peak,
            samples,
        }
    }

    #[test]
    fn suggestion_is_the_rounded_peak_times_safety_within_floor_and_max() {
        let r = rules();
        // 60 KiB × 1.5 = 90 KiB -> 96 KiB
        assert_eq!(suggest(&m("a", 60 * 1024, 5000), &r), 96 * 1024);
        // few samples: × 2 = 120 KiB -> 128 KiB
        assert_eq!(suggest(&m("a", 60 * 1024, 10), &r), 128 * 1024);
        // tiny peak: the floor
        assert_eq!(suggest(&m("a", 4 * 1024, 5000), &r), 64 * 1024);
        // huge peak: the max
        assert_eq!(suggest(&m("a", 400 * 1024, 5000), &r), 256 * 1024);
    }

    #[test]
    fn decisions_follow_the_rules() {
        let r = rules();
        assert_eq!(
            decide(128 * 1024, None, &r).1,
            Action::Keep("no measurements")
        );
        // 256 KiB current, peak 60 KiB (suggest 96 KiB): well past the hysteresis, lowered
        assert_eq!(
            decide(256 * 1024, Some(&m("a", 60 * 1024, 5000)), &r).1,
            Action::Set(96 * 1024)
        );
        // few samples never lower
        assert_eq!(
            decide(256 * 1024, Some(&m("a", 60 * 1024, 10)), &r).1,
            Action::Keep("too few samples to lower")
        );
        // 112 KiB current vs 96 KiB suggested: within 25 %, and 112/60 >= 1.5
        assert_eq!(
            decide(112 * 1024, Some(&m("a", 60 * 1024, 5000)), &r).1,
            Action::Keep("within hysteresis")
        );
        // 80 KiB current, peak 60 KiB: 80/60 < 1.5, raised although within hysteresis
        assert_eq!(
            decide(80 * 1024, Some(&m("a", 60 * 1024, 5000)), &r).1,
            Action::Set(96 * 1024)
        );
    }

    const SPEC: &str = "openapi: 3.1.0
info: { title: T, version: '1' }
x-brrtrouter-stack-size: 256KiB
paths:
  /a:
    get:
      # the list
      operationId: list_a
      responses: { '200': { description: ok } }
  /b:
    post:
      operationId: \"make_b\"
      x-brrtrouter-stack-size: 64KiB # hand-set
      responses: { '200': { description: ok } }
";

    #[test]
    fn set_in_text_adds_or_replaces_the_key_and_keeps_the_rest() {
        let added = set_in_text(SPEC, "list_a", "96KiB").unwrap();
        assert!(added.contains(
            "      operationId: list_a\n      x-brrtrouter-stack-size: \"96KiB\"\n      responses"
        ));
        assert!(added.contains("      # the list\n"));
        let replaced = set_in_text(SPEC, "make_b", "128KiB").unwrap();
        assert!(replaced.contains("      x-brrtrouter-stack-size: \"128KiB\"\n"));
        assert!(!replaced.contains("64KiB # hand-set"));
        assert_eq!(replaced.lines().count(), SPEC.lines().count());
        assert!(set_in_text(SPEC, "nope", "1KiB").is_none());
        let diff = unified_diff(Path::new("s.yaml"), SPEC, &added);
        assert!(diff.contains("+      x-brrtrouter-stack-size: \"96KiB\""));
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "brrtr_stacks_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn plan_reads_current_sizes_and_writes_only_changes() {
        let dir = temp("plan");
        let spec = dir.join("openapi.yaml");
        std::fs::write(&spec, SPEC).unwrap();
        let ms = [
            m("list_a", 60 * 1024, 5000),
            m("make_b", 60 * 1024, 5000),
            m("gone", 1, 1),
        ];
        let report = plan(&ms, &spec, &rules(), StackSizeTarget::Spec, None).unwrap();
        // list_a inherits the root 256 KiB and is lowered; make_b has 64 KiB for a 60 KiB peak and is raised
        let row = |h: &str| report.rows.iter().find(|r| r.handler == h).unwrap().clone();
        assert_eq!(row("list_a").current, 256 * 1024);
        assert_eq!(row("list_a").action, Action::Set(96 * 1024));
        assert_eq!(row("make_b").action, Action::Set(96 * 1024));
        assert!(report.warnings.iter().any(|w| w.starts_with("gone:")));
        let (_, new) = &report.edits[&spec];
        assert_eq!(new.matches("x-brrtrouter-stack-size: \"96KiB\"").count(), 2);
        // nothing written without --write
        assert_eq!(std::fs::read_to_string(&spec).unwrap(), SPEC);
        report.write().unwrap();
        assert_eq!(std::fs::read_to_string(&spec).unwrap(), *new);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn suite_config_target_writes_into_the_sub_spec() {
        let dir = temp("suite");
        std::fs::create_dir_all(dir.join("openapi/svc")).unwrap();
        let sub = dir.join("openapi/svc/openapi.yaml");
        std::fs::write(
            &sub,
            "openapi: 3.1.0\npaths:\n  /a:\n    get:\n      operationId: list_a\n      responses: {}\n",
        )
        .unwrap();
        let bff = dir.join("openapi_bff.yaml");
        std::fs::write(
            &bff,
            "openapi: 3.1.0\npaths:\n  /svc/a:\n    get:\n      operationId: list_a\n      x-service: svc\n      responses: {}\n",
        )
        .unwrap();
        let config = dir.join("bff-suite-config.yaml");
        std::fs::write(
            &config,
            "openapi_base_dir: openapi\nservices:\n  svc:\n    base_path: /svc\n",
        )
        .unwrap();
        let report = plan(
            &[m("list_a", 100 * 1024, 5000)],
            &bff,
            &rules(),
            StackSizeTarget::SuiteConfig,
            Some((&config, &dir)),
        )
        .unwrap();
        assert!(report.edits.contains_key(&sub), "{:?}", report.edits.keys());
        assert!(!report.edits.contains_key(&bff));
        assert!(report.edits[&sub]
            .1
            .contains("x-brrtrouter-stack-size: \"160KiB\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_suggestion_at_the_maximum_is_flagged() {
        let dir = temp("max");
        let spec = dir.join("openapi.yaml");
        std::fs::write(&spec, SPEC).unwrap();
        let report = plan(
            &[m("make_b", 300 * 1024, 5000)],
            &spec,
            &rules(),
            StackSizeTarget::Spec,
            None,
        )
        .unwrap();
        assert!(report
            .rows
            .iter()
            .any(|r| r.handler == "make_b" && r.at_max));
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("BRRTR_STACK_MAX_BYTES")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
