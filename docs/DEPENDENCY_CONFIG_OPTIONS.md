# BRRTRouter Dynamic Dependency Configuration Options

## Problem Statement

Hardcoding dependencies like `rust_decimal` and `rusty-money` in BRRTRouter templates is not scalable. We need a way to dynamically configure which dependencies should be included in generated `Cargo.toml` files based on:
1. What types are actually used in the generated code
2. What dependencies are available in the workspace
3. Project-specific requirements

## Proposed Options

### Option 1: Workspace Auto-Detection (Recommended - Already Partially Implemented)

**How it works:**
- BRRTRouter scans the workspace `Cargo.toml` for `[workspace.dependencies]`
- Any dependency found there is automatically included in generated `Cargo.toml` files
- No configuration needed - works out of the box

**Pros:**
- ✅ Zero configuration
- ✅ Automatically stays in sync with workspace
- ✅ Already partially implemented
- ✅ Works for all projects using the same workspace

**Cons:**
- ❌ Requires workspace structure
- ❌ May include unused dependencies (but they're optional/workspace-managed)

**Implementation:**
```rust
// Enhanced version: scan ALL workspace.dependencies, not just specific ones
fn detect_workspace_dependencies(output_dir: &Path) -> HashSet<String> {
    // Find workspace Cargo.toml
    // Parse [workspace.dependencies] section
    // Return set of dependency names
}
```

### Option 2: Configuration File (brrtrouter.toml)

**How it works:**
- Create a `brrtrouter.toml` config file in the project root or alongside OpenAPI spec
- Lists dependencies to include in generated Cargo.toml files
- Can specify workspace vs direct dependencies

**Example `brrtrouter.toml`:**
```toml
[dependencies]
# Always include these in generated Cargo.toml
rust_decimal = { workspace = true }
rusty-money = { workspace = true, features = ["serde"] }

# Or for non-workspace:
# rust_decimal = "1.33"
# rusty-money = { version = "0.5", features = ["serde"] }

# Conditional dependencies based on detected usage
[conditional]
# Include if rust_decimal::Decimal is detected in generated code
rust_decimal = { workspace = true, detect = "rust_decimal::Decimal" }
rusty-money = { workspace = true, detect = "rusty_money::Money" }
```

**Pros:**
- ✅ Explicit and clear
- ✅ Version controlled
- ✅ Can be shared across projects
- ✅ Supports conditional inclusion

**Cons:**
- ❌ Requires maintaining config file
- ❌ Another file to manage

**CLI Usage:**
```bash
brrtrouter-gen generate --spec openapi.yaml --output my-service --config brrtrouter.toml
```

### Option 3: OpenAPI Extension (x-brrtrouter-dependencies)

**How it works:**
- Add extension to OpenAPI spec to declare dependencies
- BRRTRouter reads this during code generation

**Example in `openapi.yaml`:**
```yaml
openapi: 3.0.0
info:
  title: My API
  version: 1.0.0
x-brrtrouter-dependencies:
  rust_decimal:
    workspace: true
  rusty-money:
    workspace: true
    features: ["serde"]
```

**Pros:**
- ✅ Co-located with API spec
- ✅ Per-API configuration
- ✅ Version controlled with spec

**Cons:**
- ❌ Pollutes OpenAPI spec
- ❌ Not standard OpenAPI
- ❌ Harder to share across multiple specs

### Option 4: CLI Arguments

**How it works:**
- Pass dependencies via command line flags

**CLI Usage:**
```bash
brrtrouter-gen generate \
  --spec openapi.yaml \
  --output my-service \
  --dependency rust_decimal:workspace \
  --dependency "rusty-money:workspace:features=serde"
```

**Pros:**
- ✅ Flexible for one-off generation
- ✅ No files needed

**Cons:**
- ❌ Not version controlled
- ❌ Hard to maintain for multiple services
- ❌ Verbose command lines

### Option 5: Hybrid Approach (Recommended)

**Combine Options 1 + 2:**

1. **Auto-detect from workspace** (Option 1) - Default behavior
2. **Config file override** (Option 2) - For custom requirements
3. **Type-based detection** - Scan generated code for type usage

**Priority order:**
1. Config file (if exists) - explicit override
2. Type detection - scan generated code for `rust_decimal::Decimal`, `rusty_money::Money`, etc.
3. Workspace auto-detection - fallback to workspace dependencies

**Implementation Structure:**
```rust
pub struct DependencyConfig {
    /// Dependencies to always include
    pub always_include: HashMap<String, DependencySpec>,
    /// Dependencies to include if types are detected
    pub conditional: HashMap<String, ConditionalDependency>,
    /// Whether to auto-detect from workspace
    pub auto_detect_workspace: bool,
}

pub struct ConditionalDependency {
    /// Type pattern to detect (e.g., "rust_decimal::Decimal")
    pub detect_type: String,
    /// Dependency specification
    pub spec: DependencySpec,
}
```

## Recommended Implementation Plan

### Phase 1: Type-Based Auto-Detection + Workspace Detection ✅ (IMPLEMENTED)
- ✅ Created `DependencyRegistry` to map type patterns → dependency names
- ✅ Auto-detect dependencies by scanning generated code for type patterns
- ✅ Enhanced workspace detection to scan ALL `[workspace.dependencies]`
- ✅ Include dependencies when:
  1. Types are detected in generated code (via `DependencyRegistry`)
  2. Dependencies exist in workspace (auto-detected)
- This solves 80% of use cases with zero configuration

**Current Implementation:**
- `DependencyRegistry` with mappings for `rust_decimal` and `rusty-money`
- Type scanning in `generate.rs` detects usage
- Workspace detection checks for dependencies in `[workspace.dependencies]`
- Templates conditionally include dependencies

**To Add New Dependencies:**
Simply add to `DependencyRegistry::default()`:
```rust
registry.register("chrono::DateTime", "chrono");
registry.register("uuid::Uuid", "uuid");
```

### Phase 2: Config File Support (Future Enhancement)
- Add `--config` flag to CLI
- Support `brrtrouter.toml` config file
- Allow explicit dependency declarations
- Override or supplement `DependencyRegistry` mappings

### Phase 3: OpenAPI Extension Support (Future Enhancement)
- Support `x-brrtrouter-dependencies` in OpenAPI specs
- Per-spec dependency configuration

## Current Implementation ✅

The solution has been implemented using a **DependencyRegistry** pattern:

### 1. DependencyRegistry (`src/generator/templates.rs`)

```rust
pub struct DependencyRegistry {
    type_to_dependency: HashMap<&'static str, &'static str>,
}

impl Default for DependencyRegistry {
    fn default() -> Self {
        let mut registry = Self { ... };
        // Register type → dependency mappings
        registry.register("rust_decimal::Decimal", "rust_decimal");
        registry.register("rusty_money::Money", "rusty-money");
        // Add more mappings here as needed
        registry
    }
}
```

### 2. Type-Based Detection (`src/generator/project/generate.rs`)

```rust
// Scan all generated types for dependency usage
let registry = DependencyRegistry::default();
let mut detected_deps = HashSet::new();

// Check schema types and route fields
for type_def in schema_types.values() {
    for field in &type_def.fields {
        let deps = registry.detect_from_types(&field.ty);
        detected_deps.extend(deps);
    }
}
```

### 3. Workspace Auto-Detection (`src/generator/templates.rs`)

```rust
// Automatically detect all dependencies in workspace.dependencies
let workspace_deps = detect_workspace_dependencies(output_dir);
// Returns HashSet of all dependency names found in workspace
```

### 4. Template Integration

Templates conditionally include dependencies based on:
- Type detection (via `DependencyRegistry`)
- Workspace availability (via `detect_workspace_dependencies`)

## Adding New Dependencies

To add support for a new dependency (e.g., `chrono`):

1. **Add to DependencyRegistry** (`src/generator/templates.rs`):
   ```rust
   impl Default for DependencyRegistry {
       fn default() -> Self {
           // ...
           registry.register("chrono::DateTime", "chrono");
           registry.register("chrono::NaiveDate", "chrono");
           registry
       }
   }
   ```

2. **Add to template** (`templates/Cargo.toml.txt`):
   ```toml
   {% if has_chrono %}
   chrono = { workspace = true }
   {% endif %}
   ```

3. **Add to template data struct** (`CargoTomlTemplateData`):
   ```rust
   pub has_chrono: bool,
   ```

4. **Update detection logic** to check for `chrono` in detected_deps

**Note**: Future enhancement could make templates fully dynamic to avoid step 2-4.

## Migration Path

1. **Immediate**: Revert hardcoded `rust_decimal`/`rusty-money` detection
2. **Implement**: Enhanced workspace auto-detection + type-based detection
3. **Future**: Add config file support for edge cases

This approach:
- ✅ Solves the immediate problem (no hardcoding)
- ✅ Works for most use cases (workspace + type detection)
- ✅ Extensible for future needs (config file)
- ✅ Backward compatible (existing code continues to work)
