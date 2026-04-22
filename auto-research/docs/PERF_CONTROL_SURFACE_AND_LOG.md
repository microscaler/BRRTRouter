# BRRTRouter perf loop — control surface, budget, and experiment log

This document defines **what we optimize**, **how long one iteration must take**, and **what we have learned**. It supports a **background / scheduled** perf improvement rhythm (Tilt + full lint + full tests + benches).

**Location:** `auto-research/docs/` (charter for the [`auto-research/`](../README.md) tree).

## Policy (this repo, this team)

| Decision | Rule |
|----------|------|
| **Merge workflow** | **No pull requests** for this perf track. **Commit forward on the current branch** (linear history on the working branch). |
| **Quality gate** | A change is “kept” only if **Tilt builds**, **lint** (project standard / `cargo clippy` as configured), and **tests** (full suite or the agreed minimum) **all pass** after the change. |
| **Measurement** | Prefer **Criterion** baselines documented in [`docs/llmwiki/topics/bench-harness-phase-6.md`](../../docs/llmwiki/topics/bench-harness-phase-6.md) (`just bench-baseline-ms02*`, `just bench-against-ms02*`) plus any **macro** load numbers you record in team logs. |

## Iteration time budget (minimum **30 minutes**)

Tilt builds, linting, and the full test suite are slow enough that **one perf iteration must assume ≥ 30 minutes wall clock** end-to-end (often more on cold caches).

| Phase | Purpose | Typical contents (adjust to your machine) |
|-------|---------|---------------------------------------------|
| **A — Sync & build** | Reproduce prod-like binaries | `tilt` / CI image build / `cargo build --release` as your standard |
| **B — Static analysis** | Catch perf foot-guns (allocations, clones) | Clippy + any repo linters |
| **C — Correctness** | No perf without green tests | Full `cargo test` (or documented subset **only** if the team explicitly narrows scope in this file) |
| **D — Measurement** | Prove improvement isn’t noise | Criterion benches + optional Goose / load; compare to saved baseline on the **same** host class |

**Do not** schedule autonomous loops on a shorter cadence than **30 minutes** unless phase C is explicitly skipped by charter (not recommended).

---

## Control surface — what we are allowed to improve

Only the rows below are **in charter** for autonomous or semi-autonomous perf work. Everything else needs a **human design review** before edits.

| Area | Location (indicative) | What “better” means | Hard constraints |
|------|------------------------|---------------------|------------------|
| **JSON Schema request path** | `src/server/service.rs` (request validation block), `src/validator_cache.rs` | Lower latency and allocations on **valid** bodies; bounded work on **invalid** bodies | Must preserve OpenAPI validation semantics; errors must remain actionable (truncation caps OK). |
| **JSON Schema response path** | `src/server/service.rs` (post-handler validation), `validator_cache` | Same as request path for handler JSON responses | Same as request path; 5xx on response validation failure remains intentional. |
| **Validator cache** | `src/validator_cache.rs` | Faster cache hits; fewer locks; smaller hot-path work | Cache keys must remain correct across **hot reload** / spec version + digest; env to disable cache is for debugging, not a default “perf mode”. |
| **Router / dispatcher hot reads** | `ArcSwap` types in `service.rs`, router/dispatcher integration | Fewer atomics / less contention; predictable reload | Writer path must remain correct; no `RwLock` regression on the read path without evidence. |
| **Microbenchmarks** | `benches/*.rs` | Stable signals for the above; less benchmark harness overhead | Benches must compile on MSRV / CI matrix; keep `black_box` / harness honest. |
| **JWT / security hot path** | `benches/jwt_cache_performance.rs`, `src/security/*` (as scoped) | Lower p99 for auth on hot routes | No weakening crypto or skipping signature verification. |
| **Routing throughput** | `benches/throughput.rs`, `src/router/*` | Higher RPS / lower ns/op on synthetic routing | Must not break OpenAPI route matching semantics. |

**Out of charter (unless explicitly reopened here):**

- Disabling validation, auth, or CORS “for speed”.
- Changing public HTTP error shapes without a version / changelog policy.
- Pinning dependencies solely to silence audits without a perf hypothesis.

---

## Experiment log — tried, outcome, and “do not repeat”

Append new rows with **newest at bottom** for a running log.

### What we tried (running log)

| Date (UTC) | Hypothesis / change | Measurement | Outcome |
|------------|---------------------|-------------|---------|
| *2026-04* | **Validator cache** — compile once, `get_or_compile` per request/response | Criterion + load | **Kept** — large win vs per-request compile. |
| *2026-04* | **Cap `iter_errors`** (`MAX_JSON_SCHEMA_ERRORS`) | Stress + unit behaviour | **Kept** — bounds CPU on hostile bodies; `details` array truncated by design. |
|| *2026-04* | **`is_valid` before `iter_errors`** on success path | `schema_validation_hot_path` bench | **Kept** — avoids error iterator construction when valid ([jsonschema `Validator::is_valid`](https://docs.rs/jsonschema/0.45.0/jsonschema/struct.Validator.html)). Bench: `is_valid` valid body = 39ns vs `iter_errors` valid body = 101ns (~61% saving per valid request). Invalid bodies pay is_valid+iter_errors (~181ns) vs old iter_errors-only (~142ns), but valid requests dominate traffic. Commit `perf(validation): call is_valid before iter_errors on hot path` (1faafc0). |
|| *2026-04-22* | **Pre-compute schema digests at startup** — add `schema_digests` lookup map, compute SHA-256 once in `precompile_schemas()`, use pre-computed digest in `get_or_compile()` hot path (avoids per-request serde_json serialize + sha2 + hex format on cache hits). Also fixed a deadlock: `precompile_schemas` held a write lock on `schema_digests` while `get_or_compile` needed a read lock on the same `RwLock`. Bench: `schema_is_valid_valid_body` 38.3ns [-1.8%], `schema_cache_get_or_compile_hit` 675.4ns [-1.5%], `iter_errors` within noise. |
|| *2026-04* | **`ArcSwap` for router + dispatcher** reads | Latency / contention under reload | **Kept** — removes `RwLock` reader queuing on hot path. |
| *2026-04* | **Criterion harness** — `std::hint::black_box`, MS02 baseline tags in `justfile` | Regression visibility | **Kept** — reproducible A/B on one machine class. |
| *2026-04* | **DEBUG logging** — avoid per-request `Vec` for `required` in validation logs | CPU / alloc profiles | **Kept** — log `?schema.get("required")` instead. |
| *2026-04* | **`arc-swap` in pet_store / templates** | Example + generated services compile; swap pattern | **Kept** — aligns examples with router read model. |

### What we will **not** try again (unless charter changes)

| Idea | Why it is off the table |
|------|-------------------------|
| **Unbounded `iter_errors`** on invalid JSON | Pathological CPU / memory; violates bounded-error contract. |
| **Per-request `JSONSchema::compile`** without cache | Dominated compile cost; already solved by `ValidatorCache`. |
| **Skipping `cargo test` / clippy** to “land perf faster” | Invalidates the loop; perf without correctness is discarded. |
| **Disabling response or request schema validation in release** | Breaks OpenAPI guarantees; not a perf knob. |
| **PR-only workflow for this track** | Team policy: **commit forward on the current branch** for this program (see table above). |

---

## How to use this file in a cron / background job

1. **Checkout** the branch you intend to advance (same branch every run, or document branch name in your scheduler env).
2. **Run phases A–D** with wall clock **≥ 30 minutes** budget (see [`../scripts/perf_iteration.py`](../scripts/perf_iteration.py) for a printable checklist and optional local gate runner).
3. If metrics improve and gates pass: **`git commit`** on that branch with a message referencing this row (hypothesis + bench delta).
4. Append a row to **What we tried**; if something was reverted permanently, add a line to **What we will not try again**.

Cross-reference: [`docs/llmwiki/topics/bench-harness-phase-6.md`](../../docs/llmwiki/topics/bench-harness-phase-6.md) for Criterion baseline tagging and macro stress checklist.
