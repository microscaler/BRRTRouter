# Core Docs vs Codebase Reconciliation

- Status: partially-verified

## Baseline validation context (pre-existing)
- `cargo fmt --all -- --check`: pass
- `cargo build --workspace`: pass
- `cargo clippy --workspace --all-targets --all-features`: fails on existing deny-lint in `src/server/request.rs` (`clippy::unnecessary_to_owned`)
- `cargo test --workspace -- --nocapture`: fails in `curl_integration_tests` due to missing `x86_64-unknown-linux-musl` target in environment

## Reconciled documents

| Doc | Reconciled status | Notes |
|---|---|---|
| `docs/ARCHITECTURE.md` | partially-verified | Major flow is directionally correct; several implementation details need signature-level updates |
| `docs/DEVELOPMENT.md` | partially-verified | `just` workflow is documented but `just` was unavailable in this environment; cargo commands still valid |
| `docs/TEST_DOCUMENTATION.md` | partially-verified | Test categories align broadly; exact counts/coverage claims need recheck against current suite |
| `docs/CORS_OPERATIONS.md` | verified | Dedicated reconciliation completed in `llmwiki/reconciliation/cors-operations-vs-codebase.md` |
| `docs/PERFORMANCE.md` | pending | Requires benchmark and load-test artifact reconciliation |

## Notable drift to fix next
1. Some architecture prose implies `load_spec` returns parsed spec objects; current signature returns route metadata + slug.
2. Test-doc numeric assertions should be regenerated from current `tests/` tree + CI targets.
3. Operational docs should include explicit fallback commands when `just` is not installed.
