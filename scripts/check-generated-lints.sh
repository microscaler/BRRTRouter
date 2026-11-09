#!/usr/bin/env bash
# Generated code must pass a strict consumer's lints.
#
# Builds a throwaway workspace under target/generated-lints that declares the strictest [workspace.lints] a BRRTRouter
# consumer uses today (RERP's: clippy pedantic plus no unwrap/expect/panic/print/float arithmetic, a reason on every
# allow, a SAFETY comment on every unsafe block), generates the pet_store gen crate and impl stubs into it, and runs
# clippy -D warnings. The impl crate opts into the lints by itself (the generator writes `[lints] workspace = true`
# when the workspace declares lints); the gen crate keeps clippy's defaults, as in every consumer.
#
# Usage: scripts/check-generated-lints.sh [spec]   (default examples/openapi.yaml; run from the repository root)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPEC="${1:-$ROOT/examples/openapi.yaml}"
W="$ROOT/target/generated-lints"
GEN="${BRRTROUTER_GEN:-$ROOT/target/debug/brrtrouter-gen}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

[ -x "$GEN" ] || cargo build --manifest-path "$ROOT/Cargo.toml" --bin brrtrouter-gen
rm -rf "$W"
mkdir -p "$W/svc"

# 1. A bare workspace: the gen crate is generated with path dependencies on this checkout.
cat > "$W/Cargo.toml" <<'TOML'
[workspace]
resolver = "2"
members = ["svc/*"] # a glob: the impl crate joins once generate-stubs has written it
TOML
"$GEN" generate --spec "$SPEC" --output "$W/svc/gen" --force >/dev/null

# 2. The strict lints (copied from RERP's microservices/Cargo.toml) and the workspace dependencies the impl template
#    uses, then the impl stubs: the generator sees [workspace.lints] and opts the impl crate in.
cat >> "$W/Cargo.toml" <<TOML

[workspace.lints.rust]
rust_2018_idioms = { level = "warn", priority = -1 }
unused_qualifications = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
needless_pass_by_value = "allow"
pub_underscore_fields = "allow"
similar_names = "allow"
items_after_statements = "allow"
default_trait_access = "allow"
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
todo = "warn"
unimplemented = "warn"
dbg_macro = "warn"
float_arithmetic = "warn"
let_underscore_must_use = "warn"
allow_attributes_without_reason = "warn"
undocumented_unsafe_blocks = "warn"
print_stdout = "warn"
print_stderr = "warn"

[workspace.dependencies]
brrtrouter = { path = "$ROOT" }
brrtrouter_macros = { path = "$ROOT/brrtrouter_macros" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tikv-jemallocator = { version = "0.7", features = ["profiling"] }
clap = { version = "4.6", features = ["derive"] }
may = "0.3"
TOML
"$GEN" generate-stubs --spec "$SPEC" --output "$W/svc/impl" --component-name "$(sed -n 's/^name = "\(.*\)"/\1/p' "$W/svc/gen/Cargo.toml" | head -1)" --force >/dev/null

grep -q '^\[lints\]' "$W/svc/impl/Cargo.toml" || { echo "impl Cargo.toml did not opt into [workspace.lints]"; exit 1; }

# 3. Gen crate under default lints, impl crate under the strict set; every warning fails.
cd "$W"
cargo clippy --all-targets -- -D warnings
echo "generated code passes the strict lint set"
