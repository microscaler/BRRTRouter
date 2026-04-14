# BRRTRouter Dependency Registry - Usage Guide

## Overview

BRRTRouter now uses a **DependencyRegistry** system to automatically detect and include dependencies in generated `Cargo.toml` files. This eliminates the need to hardcode dependencies for every use case.

## How It Works

### 1. Type-Based Detection

The `DependencyRegistry` maps Rust type patterns to Cargo dependency names:

```rust
// In src/generator/templates.rs
pub struct DependencyRegistry {
    type_to_dependency: HashMap<&'static str, &'static str>,
}
```

**Current Mappings:**
- `rust_decimal::Decimal` → `rust_decimal`
- `rusty_money::Money` → `rusty-money`
- `chrono::` → `chrono` (partial match for any chrono type)
- `uuid::Uuid` → `uuid`

### 2. Automatic Detection Process

When generating code, BRRTRouter:

1. **Scans generated types** - Checks all schema types and route fields
2. **Matches against registry** - Uses `DependencyRegistry::detect_from_types()`
3. **Checks workspace** - Verifies dependencies exist in `[workspace.dependencies]`
4. **Includes in Cargo.toml** - Conditionally adds dependencies to generated files

### 3. Workspace Integration

If using workspace dependencies:
- BRRTRouter scans `[workspace.dependencies]` in parent `Cargo.toml`
- Only includes dependencies that exist in workspace
- Automatically uses `{ workspace = true }` syntax

## Adding New Dependencies

### Step 1: Register Type Pattern

Edit `src/generator/templates.rs`:

```rust
impl Default for DependencyRegistry {
    fn default() -> Self {
        let mut registry = Self {
            type_to_dependency: HashMap::new(),
        };
        
        // Existing mappings
        registry.register("rust_decimal::Decimal", "rust_decimal");
        registry.register("rusty_money::Money", "rusty-money");
        
        // Add your new mapping
        registry.register("chrono::DateTime", "chrono");
        registry.register("chrono::NaiveDate", "chrono");
        // Use partial match for all chrono types:
        registry.register("chrono::", "chrono");
        
        registry
    }
}
```

### Step 2: Update Template (Current Limitation)

Currently, templates need explicit flags. Update `templates/Cargo.toml.txt`:

```toml
{% if has_chrono %}
chrono = { workspace = true }
{% endif %}
```

And `CargoTomlTemplateData` struct:
```rust
pub has_chrono: bool,
```

**Future Enhancement**: Make templates fully dynamic to avoid this step.

### Step 3: Update Detection Logic

In `src/generator/project/generate.rs`, add to detection:

```rust
let has_chrono = detected_deps.contains("chrono");
```

## Example: Adding `serde_with` Support

```rust
// 1. Register in DependencyRegistry
registry.register("serde_with::", "serde_with");

// 2. Update template (until dynamic templates are implemented)
// In Cargo.toml.txt:
{% if has_serde_with %}
serde-with = { workspace = true, features = ["chrono"] }
{% endif %}

// 3. Update template data
pub has_serde_with: bool,

// 4. Update detection
let has_serde_with = detected_deps.contains("serde-with");
```

## Future Enhancements

### Dynamic Template Generation

Instead of hardcoding each dependency flag, templates could iterate over detected dependencies:

```toml
{% for dep in detected_dependencies %}
{{ dep.name }} = {{ dep.spec }}
{% endfor %}
```

This would eliminate steps 2-4 above.

### Configuration File Support

Add `brrtrouter.toml` to override or supplement registry:

```toml
[dependencies]
# Always include
chrono = { workspace = true }

[conditional]
# Include if type detected
serde_with = { workspace = true, detect = "serde_with::" }
```

## Current Status

✅ **Implemented:**
- `DependencyRegistry` with type-based detection
- Workspace dependency auto-detection
- Support for `rust_decimal` and `rusty-money`

🔄 **In Progress:**
- Making templates more dynamic to reduce boilerplate

📋 **Future:**
- Config file support
- OpenAPI extension support
- Fully dynamic template generation
