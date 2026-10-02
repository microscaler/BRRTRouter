# BRRTRouter Dynamic Dependency System - Implementation Summary

## ✅ What Was Implemented

A **DependencyRegistry** system that automatically detects and includes dependencies in generated `Cargo.toml` files without hardcoding them for every use case.

## Architecture

### 1. DependencyRegistry (`src/generator/templates.rs`)

A registry that maps Rust type patterns to Cargo dependency names:

```rust
pub struct DependencyRegistry {
    type_to_dependency: HashMap<&'static str, &'static str>,
}
```

**Current Mappings:**
- `rust_decimal::Decimal` → `rust_decimal`
- `rusty_money::Money` → `rusty-money`
- `chrono::` → `chrono` (partial match)
- `uuid::Uuid` → `uuid`

### 2. Type-Based Detection (`src/generator/project/generate.rs`)

Scans all generated types and route fields to detect which dependencies are needed:

```rust
let registry = DependencyRegistry::default();
let mut detected_deps = HashSet::new();

// Scan schema types and route fields
for type_def in schema_types.values() {
    for field in &type_def.fields {
        let deps = registry.detect_from_types(&field.ty);
        detected_deps.extend(deps);
    }
}
```

### 3. Workspace Auto-Detection (`src/generator/templates.rs`)

Automatically detects all dependencies available in workspace:

```rust
pub(crate) fn detect_workspace_dependencies(output_dir: &Path) -> HashSet<String>
```

Scans `[workspace.dependencies]` section and returns all dependency names.

### 4. Template Integration

Templates conditionally include dependencies based on:
- Type detection results
- Workspace availability

## How It Works

1. **Code Generation**: BRRTRouter generates handlers/controllers with types
2. **Type Scanning**: Scans all generated types using `DependencyRegistry`
3. **Workspace Check**: Verifies dependencies exist in workspace (if using workspace deps)
4. **Template Rendering**: Conditionally includes dependencies in `Cargo.toml`

## Adding New Dependencies

### Simple Case: Just Add to Registry

For dependencies that follow standard patterns, just add to `DependencyRegistry::default()`:

```rust
// In src/generator/templates.rs
impl Default for DependencyRegistry {
    fn default() -> Self {
        // ...
        registry.register("my_crate::MyType", "my-crate");
        registry
    }
}
```

### Complex Case: Template Updates Needed

Currently, templates need explicit flags. To add a new dependency:

1. **Add to DependencyRegistry** (as above)
2. **Update template** (`templates/Cargo.toml.txt`):
   ```toml
   {% if has_my_crate %}
   my-crate = { workspace = true }
   {% endif %}
   ```
3. **Update template data struct** (`CargoTomlTemplateData`):
   ```rust
   pub has_my_crate: bool,
   ```
4. **Update detection logic** in `generate.rs`:
   ```rust
   let has_my_crate = detected_deps.contains("my-crate");
   ```

**Note**: Future enhancement will make templates fully dynamic to eliminate steps 2-4.

## Benefits

✅ **No Hardcoding**: Dependencies are detected, not hardcoded  
✅ **Extensible**: Easy to add new type→dependency mappings  
✅ **Workspace-Aware**: Automatically uses workspace dependencies when available  
✅ **Type-Safe**: Only includes dependencies when types are actually used  
✅ **Backward Compatible**: Existing code continues to work  

## Future Enhancements

1. **Dynamic Templates**: Make templates iterate over detected dependencies (eliminate template updates)
2. **Config File Support**: Add `brrtrouter.toml` for explicit dependency declarations
3. **OpenAPI Extension**: Support `x-brrtrouter-dependencies` in OpenAPI specs
4. **Dependency Specs**: Store full dependency specifications (version, features) in registry

## Current Limitations

- Templates still need explicit flags for each dependency (until dynamic templates are implemented)
- Only supports dependencies that can be detected via type patterns
- No support for conditional features or version constraints yet

## Migration Notes

The system is backward compatible:
- Existing code generation continues to work
- New dependencies are automatically detected
- Workspace detection ensures dependencies are available before including them
