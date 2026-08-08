# Interpreting Flamegraphs

Flamegraphs visualize CPU usage over time. Each bar represents a function call; wider bars consumed more CPU during the profiling run.

- **X-axis** – stack traces sorted by time spent. The width shows how much of the sample period was spent in that call stack.
- **Y-axis** – call depth. Parents call the functions above them.
- **Hot spots** – the widest blocks near the bottom are typically the most expensive code paths.

Use your browser's search to find functions of interest. Hovering over a block displays its percentage of total CPU time.

To reduce noise, run profiling in **release** mode and exercise the workload you care about before stopping `cargo flamegraph`.

## Validator-path profile (Story 12.8 / P4)

Phase 6 evidence says **schema validation ≫ route match**. Profile the
validation path before proposing radix changes.

### Prerequisites

```bash
cargo install flamegraph   # once
# Linux (ms02): perf permissions — may need:
#   sudo sysctl kernel.perf_event_paranoid=1
```

### 1) Micro: schema hot path only

Drive CPU in the Criterion validation bench while capturing:

```bash
cd ~/Workspace/microscaler/BRRTRouter
source ~/.cargo/env
cargo flamegraph -p brrtrouter --bench schema_validation_hot_path -- \
  --bench schema_is_valid_valid_body
```

Search the SVG for `is_valid`, `iter_errors`, `ValidatorCache`, `jsonschema`.

### 2) Comparative: match vs validate

```bash
cargo flamegraph -p brrtrouter --bench match_vs_validate
```

On the pet microbench, match / `is_valid` / `iter_errors` bars may be
**comparable** (all sub-µs). Use a **pet_store load** profile (§3) to see where
multi-ms time goes — do not treat a wide `Router::route` bar in a match-only
bench as proof that routing dominates production traffic.

### 3) Macro: pet_store under load (optional)

```bash
# Terminal A — server
RUST_LOG=brrtrouter=warn cargo run --release -p pet_store

# Terminal B — profile the server PID (adjust binary name)
cargo flamegraph --pid $(pgrep -n pet_store)

# Terminal C — load (short)
cargo run --release --example api_load_test -- \
  --host http://127.0.0.1:8080 --users 200 --run-time 30s
```

In the SVG, look for `AppService`, `get_or_compile`, `is_valid`, and dispatcher
send paths. Wide `Router::route` alone is **not** sufficient to justify a trie
rewrite — compare widths to validation (N2 / N6).

### Reading checklist

| Search term | Meaning |
|-------------|---------|
| `Router::route` / radix | Match cost — should be narrow vs validate |
| `is_valid` / `iter_errors` | Schema hot path (primary candidate) |
| `ArcSwap` / `load` | Lock-free router/dispatcher snapshot |
| `RwLock` on request path | Regression — should not appear for router match |

Output: `flamegraph.svg` in the current directory (open in a browser).

## General one-liner

```bash
cargo flamegraph -p brrtrouter
```
