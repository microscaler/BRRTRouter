//! `brrtrouter-gen stack-sizes`: each handler's `x-brrtrouter-stack-size` from measured peak stack use.
//!
//! Input is one service's measurements, e.g. `[{"handler": "list_a", "peak_bytes": 65536, "samples": 1000}]`, for example
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
//! - a suggestion at `--max` is flagged: the runtime clamps to its `BRRTR_STACK_MAX_BYTES`, so it must be raised
//!   first. `--max` defaults to this shell's `BRRTR_STACK_MAX_BYTES`, else 256 KiB; it must match the deployed one.
//!
//! `current` is what the runtime resolves from the spec: the operation's value (`x-brrtrouter-stack-size`, else the
//! `x-stack-size` and `x-x-brrtrouter-stack-size` spellings the runtime also reads), else the spec root's, else the
//! 256 KiB fallback, clamped to `--max`. A `BRRTR_STACK_SIZE__<HANDLER>` override in the deployed environment wins
//! over the spec and is not visible here; one set in this shell is reported. Without `--write` the command prints a table and a unified diff; with it, it
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

impl Rules {
    /// Check the rules make sense: factors at least 1 (a smaller one could size a stack below its peak),
    /// `low_sample_safety >= safety`, hysteresis finite and not negative, `round > 0`, `floor <= max`.
    ///
    /// # Errors
    ///
    /// The first rule that does not hold.
    pub fn validate(&self) -> anyhow::Result<()> {
        let factor_ok = |f: f64| f.is_finite() && f >= 1.0;
        anyhow::ensure!(
            factor_ok(self.safety),
            "--safety must be a number >= 1, got {}",
            self.safety
        );
        anyhow::ensure!(
            factor_ok(self.low_sample_safety) && self.low_sample_safety >= self.safety,
            "--low-sample-safety must be a number >= --safety, got {}",
            self.low_sample_safety
        );
        anyhow::ensure!(
            self.hysteresis.is_finite() && self.hysteresis >= 0.0,
            "--hysteresis must be a number >= 0, got {}",
            self.hysteresis
        );
        anyhow::ensure!(self.round > 0, "--round must be more than 0");
        anyhow::ensure!(
            self.floor <= self.max,
            "--floor ({}) must not exceed --max ({})",
            self.floor,
            self.max
        );
        Ok(())
    }
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
    // The keys the runtime reads, in its order (`spec::build`: oas3 strips one `x-` from extension names).
    let size = |v: &serde_yaml::Value, keys: &[&str]| {
        keys.iter()
            .find_map(|k| v.get(*k))
            .and_then(|v| size_from_value(&json(v)))
    };
    let root = size(spec, &[KEY, "x-x-brrtrouter-stack-size"]);
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
                    own: size(op, &[KEY, "x-stack-size", "x-x-brrtrouter-stack-size"]),
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

/// The `# comment` after the scalar on a block-style `key: value` line (from `#`), if any; a quoted scalar is
/// skipped first, so a `#` inside it is not a comment.
fn trailing_comment(value: &str) -> Option<&str> {
    let v = value.trim_start();
    let rest = ['"', '\'']
        .into_iter()
        .find_map(|q| {
            let inner = v.strip_prefix(q)?;
            Some(inner.find(q).map_or("", |end| &inner[end + 1..]))
        })
        .map_or(v, |rest| rest);
    rest.char_indices()
        .find(|&(j, c)| c == '#' && rest[..j].ends_with([' ', '\t']))
        .map(|(j, _)| &rest[j..])
}

/// The lines of the top-level `paths:` block, as a range of line indexes.
fn paths_block(lines: &[&str]) -> Option<std::ops::Range<usize>> {
    let start = lines.iter().position(|l| l.trim_end() == "paths:")? + 1;
    let len = lines[start..]
        .iter()
        .take_while(|l| l.trim().is_empty() || indent(l) > 0 || l.starts_with('#'))
        .count();
    Some(start..start + len)
}

/// The scalar on a block-style `key: value` line: quoted (`"a"`, `'a'`) or plain, without a trailing `# comment`.
fn plain_scalar(value: &str) -> &str {
    let v = value.trim_start();
    for q in ['"', '\''] {
        if let Some(rest) = v.strip_prefix(q) {
            return rest.find(q).map_or(rest, |end| &rest[..end]);
        }
    }
    let end = v
        .char_indices()
        .find(|&(i, c)| c == '#' && v[..i].ends_with([' ', '\t']))
        .map_or(v.len(), |(i, _)| i);
    v[..end].trim()
}

/// Set `x-brrtrouter-stack-size: <value>` on the operation whose `operationId` is `op_id`, editing the YAML text so
/// comments and layout are kept: the key is replaced when the operation has it (keeping a trailing comment), else
/// added after `operationId`. Only the top-level `paths:` block is searched, so an example elsewhere (under
/// `components`, say) is never edited. `None` unless exactly one block-style `operationId: <op_id>` line is found
/// there: none (a flow-style operation, `get: { operationId: ... }`) or several would make the edit unsafe.
#[must_use]
pub fn set_in_text(text: &str, op_id: &str, value: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let block = paths_block(&lines)?;
    let mut found = lines
        .iter()
        .enumerate()
        .skip(block.start)
        .take(block.len())
        .filter(|(_, l)| {
            l.trim_start()
                .strip_prefix("operationId:")
                .map(plain_scalar)
                == Some(op_id)
        });
    let (at, _) = found.next()?;
    if found.next().is_some() {
        return None;
    }
    let ind = indent(lines[at]);
    let in_block = |l: &str| l.trim().is_empty() || indent(l) >= ind;
    let start = at - lines[..at].iter().rev().take_while(|l| in_block(l)).count();
    let end = at + 1 + lines[at + 1..].iter().take_while(|l| in_block(l)).count();
    let pad = " ".repeat(ind);
    let new_line = format!("{pad}{KEY}: \"{value}\"");
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_string()).collect();
    let key = format!("{KEY}:");
    match (start..end).find(|&i| indent(lines[i]) == ind && lines[i].trim_start().starts_with(&key))
    {
        Some(i) => {
            out[i] = match trailing_comment(&lines[i].trim_start()[key.len()..]) {
                Some(c) => format!("{new_line} {c}"),
                None => new_line,
            };
        }
        None => out.insert(at + 1, new_line),
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    Some(s)
}

/// A unified diff of `old` → `new` for edits that replace or insert single lines (what [`set_in_text`] does), with
/// two lines of context; changes whose context overlaps share a hunk, so `git apply` accepts the result.
#[must_use]
pub fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    const CTX: usize = 2;
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    // The aligned lines: ' ' kept, '-' removed, '+' added.
    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while j < b.len() {
        if i < a.len() && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
        } else if i < a.len() && b.get(j + 1) == Some(&a[i]) {
            ops.push(('+', b[j]));
        } else {
            if let Some(r) = a.get(i).copied() {
                ops.push(('-', r));
                i += 1;
            }
            ops.push(('+', b[j]));
        }
        j += 1;
    }
    ops.extend(a[i.min(a.len())..].iter().map(|l| ('-', *l)));
    // Hunks: ranges of `ops` around the changes, merged when they touch.
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for k in (0..ops.len()).filter(|&k| ops[k].0 != ' ') {
        let (lo, hi) = (k.saturating_sub(CTX), (k + CTX + 1).min(ops.len()));
        match hunks.last_mut() {
            Some(last) if lo <= last.1 => last.1 = hi,
            _ => hunks.push((lo, hi)),
        }
    }
    if hunks.is_empty() {
        return String::new();
    }
    let count = |ops: &[(char, &str)], side: char| {
        ops.iter().filter(|(t, _)| *t == ' ' || *t == side).count()
    };
    let mut out = format!("--- {0}\n+++ {0}\n", path.display());
    for (lo, hi) in hunks {
        let (old_len, new_len) = (count(&ops[lo..hi], '-'), count(&ops[lo..hi], '+'));
        // An empty side starts at the line before (0 at the top), as diff writes it.
        let start = |side: char, len: usize| count(&ops[..lo], side) + usize::from(len > 0);
        let _ = writeln!(
            out,
            "@@ -{},{old_len} +{},{new_len} @@",
            start('-', old_len),
            start('+', new_len)
        );
        for (t, l) in &ops[lo..hi] {
            let _ = writeln!(out, "{t}{l}");
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
    rules.validate()?;
    let mut by_handler: BTreeMap<&str, &Measurement> = BTreeMap::new();
    for m in measurements {
        anyhow::ensure!(
            by_handler.insert(m.handler.as_str(), m).is_none(),
            "{}: more than one measurement; give one record per handler",
            m.handler
        );
    }
    let spec_value: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(spec)?)?;
    let (ops, root) = operations(&spec_value);
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
        let env = format!("BRRTR_STACK_SIZE__{}", op.handler.to_uppercase());
        if std::env::var_os(&env).is_some() {
            report.warnings.push(format!(
                "{}: {env} is set in this shell; where it is set at runtime it overrides the spec size",
                op.handler
            ));
        }
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
                    "{}: not exactly one block-style `operationId: {}` in {}",
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

    /// Write the edited files: each is first written in full to a new uniquely named file beside its destination
    /// (created exclusively, so nothing existing is truncated), and only once all are
    /// written are they renamed over the originals, so a failed write leaves every spec as it was. A failure
    /// between two renames can still leave some files updated and others not; the error names the file.
    ///
    /// # Errors
    ///
    /// A file cannot be written or renamed.
    pub fn write(&self) -> anyhow::Result<()> {
        let mut staged: Vec<(PathBuf, &Path)> = Vec::new();
        let result = (|| -> anyhow::Result<()> {
            for (path, (_, new)) in &self.edits {
                use std::io::Write as _;
                let mut name = path.file_name().unwrap_or_default().to_os_string();
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                name.push(format!(".stack-sizes.{}.{nanos}.tmp", std::process::id()));
                let tmp = path.with_file_name(name);
                // create_new: never truncate or follow an existing file (or symlink) at that path.
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&tmp)
                    .map_err(|e| anyhow::anyhow!("{}: {e}", tmp.display()))?;
                staged.push((tmp.clone(), path));
                f.write_all(new.as_bytes())
                    .and_then(|()| f.sync_all())
                    .map_err(|e| anyhow::anyhow!("{}: {e}", tmp.display()))?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            for (tmp, _) in &staged {
                let _ = std::fs::remove_file(tmp);
            }
            return Err(e);
        }
        for (i, (tmp, path)) in staged.iter().enumerate() {
            if let Err(e) = std::fs::rename(tmp, path) {
                for (tmp, _) in &staged[i..] {
                    let _ = std::fs::remove_file(tmp);
                }
                anyhow::bail!(
                    "{}: {e}; {i} of {} file(s) were already updated",
                    path.display(),
                    staged.len()
                );
            }
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
        assert!(replaced.contains("      x-brrtrouter-stack-size: \"128KiB\" # hand-set\n"));
        assert_eq!(replaced.lines().count(), SPEC.lines().count());
        assert!(set_in_text(SPEC, "nope", "1KiB").is_none());
        let diff = unified_diff(Path::new("s.yaml"), SPEC, &added);
        assert!(diff.contains("+      x-brrtrouter-stack-size: \"96KiB\""));
    }

    #[test]
    fn operation_ids_with_comments_match_and_nearby_changes_share_a_hunk() {
        assert_eq!(plain_scalar(" list_a # why"), "list_a");
        assert_eq!(plain_scalar(" 'a#b' # why"), "a#b");
        assert_eq!(plain_scalar(" \"make_b\""), "make_b");
        assert_eq!(plain_scalar(" a#b"), "a#b");
        let commented = SPEC.replace("operationId: list_a", "operationId: list_a # the list op");
        assert!(set_in_text(&commented, "list_a", "96KiB").is_some());
        let both = set_in_text(
            &set_in_text(SPEC, "list_a", "96KiB").unwrap(),
            "make_b",
            "128KiB",
        )
        .unwrap();
        let diff = unified_diff(Path::new("s.yaml"), SPEC, &both);
        assert_eq!(diff.matches("@@ ").count(), 1, "{diff}");
        assert!(diff.contains("@@ -7,8 +7,9 @@\n"), "{diff}");
        let body: Vec<&str> = diff.lines().skip(2).collect();
        assert_eq!(body.len(), 11, "{diff}");
    }

    #[test]
    fn rules_duplicates_ambiguous_ids_and_other_size_keys() {
        let mut r = rules();
        assert!(r.validate().is_ok());
        r.safety = -1.0;
        assert!(r.validate().is_err());
        let mut r = rules();
        r.round = 0;
        assert!(r.validate().is_err());
        let twice = format!("{SPEC}  /c:\n    get:\n      operationId: list_a\n");
        assert!(set_in_text(&twice, "list_a", "96KiB").is_none());
        let example =
            format!("{SPEC}components:\n  examples:\n    x:\n      operationId: list_a\n");
        let out = set_in_text(&example, "list_a", "96KiB").unwrap();
        assert!(
            out.ends_with("    x:\n      operationId: list_a\n"),
            "{out}"
        );
        let other = SPEC.replace(
            "x-brrtrouter-stack-size: 64KiB # hand-set",
            "x-stack-size: 80KiB",
        );
        let (ops, root) = operations(&serde_yaml::from_str(&other).unwrap());
        assert_eq!(root, Some(256 * 1024));
        assert_eq!(
            ops.iter().find(|o| o.handler == "make_b").unwrap().own,
            Some(80 * 1024)
        );
        let spec = temp("dup").join("s.yaml");
        std::fs::write(&spec, SPEC).unwrap();
        let ms = [m("list_a", 1, 1), m("list_a", 2, 2)];
        let err = plan(&ms, &spec, &rules(), StackSizeTarget::Spec, None).unwrap_err();
        assert!(err.to_string().contains("more than one"), "{err}");
    }

    #[test]
    fn comments_after_quoted_values_flow_style_and_key_precedence() {
        for (old, comment) in [
            ("\"64KiB\" # hand-set", Some("# hand-set")),
            ("'64KiB' # hand-set", Some("# hand-set")),
            ("\"a#b\"", None),
            ("64KiB # hand-set", Some("# hand-set")),
            ("64KiB", None),
        ] {
            assert_eq!(trailing_comment(old), comment, "{old}");
        }
        let quoted = SPEC.replace("64KiB # hand-set", "\"64KiB\" # hand-set");
        let out = set_in_text(&quoted, "make_b", "128KiB").unwrap();
        assert!(
            out.contains("      x-brrtrouter-stack-size: \"128KiB\" # hand-set\n"),
            "{out}"
        );
        let flow = "openapi: 3.1.0\ncomponents:\n  examples:\n    e:\n      value:\n        operationId: list_a\npaths:\n  /a:\n    get: { operationId: list_a, responses: {} }\n";
        assert!(set_in_text(flow, "list_a", "96KiB").is_none());
        let both = SPEC.replace(
            "x-brrtrouter-stack-size: 64KiB # hand-set",
            "x-x-brrtrouter-stack-size: 96KiB\n      x-stack-size: 80KiB",
        );
        let (ops, _) = operations(&serde_yaml::from_str(&both).unwrap());
        assert_eq!(
            ops.iter().find(|o| o.handler == "make_b").unwrap().own,
            Some(80 * 1024)
        );
        let xx = SPEC.replace(
            "x-brrtrouter-stack-size: 64KiB # hand-set",
            "x-x-brrtrouter-stack-size: 96KiB",
        );
        let (ops, _) = operations(&serde_yaml::from_str(&xx).unwrap());
        assert_eq!(
            ops.iter().find(|o| o.handler == "make_b").unwrap().own,
            Some(96 * 1024)
        );
    }

    #[test]
    fn write_stages_new_files_and_a_failed_stage_changes_nothing() {
        let d = temp("write");
        let a = d.join("a.yaml");
        std::fs::write(&a, "old a\n").unwrap();
        let mut report = Report::default();
        report
            .edits
            .insert(a.clone(), ("old a\n".into(), "new a\n".into()));
        report
            .edits
            .insert(d.join("missing/b.yaml"), (String::new(), "new b\n".into()));
        assert!(report.write().is_err());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "old a\n");
        let names: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        report.edits.remove(&d.join("missing/b.yaml"));
        report.write().unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "new a\n");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
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
