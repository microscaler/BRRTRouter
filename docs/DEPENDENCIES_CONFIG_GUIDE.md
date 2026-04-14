# BRRTRouter Dependencies Configuration Guide

## Overview

BRRTRouter supports per-microservice dependency configuration via a `brrtrouter-dependencies.toml` file that sits alongside your OpenAPI specification. This allows each microservice to specify its own additional dependencies without hardcoding them in BRRTRouter.

## File Location

Place `brrtrouter-dependencies.toml` in the same directory as your `openapi.yaml`:

```
openapi/
  accounting/
    invoice/
      openapi.yaml
      brrtrouter-dependencies.toml  ← Here
    budget/
      openapi.yaml
      brrtrouter-dependencies.toml  ← Each service can have its own
```

## Auto-Detection

BRRTRouter automatically detects the config file:
- Looks for `brrtrouter-dependencies.toml` alongside the OpenAPI spec
- No CLI flag needed (unless you want to override the path)

## Manual Override

You can also specify the config file explicitly:

```bash
brrtrouter-gen generate \
  --spec openapi/accounting/invoice/openapi.yaml \
  --output microservices/accounting/invoice/gen \
  --dependencies-config openapi/accounting/invoice/brrtrouter-dependencies.toml
```

## Configuration Format

### Always-Included Dependencies

Dependencies listed under `[dependencies]` are always included in the generated `Cargo.toml`:

```toml
[dependencies]
# Workspace dependency
my-crate = { workspace = true }

# Workspace dependency with features
another-crate = { workspace = true, features = ["serde", "async"] }

# Version-based dependency (for non-workspace)
external-crate = { version = "1.0", features = ["async"] }

# Path-based dependency
local-crate = { path = "../../shared/crate" }

# Git dependency
git-crate = { git = "https://github.com/user/repo", branch = "main" }
```

### Conditional Dependencies

Dependencies listed under `[conditional]` are only included if the specified type pattern is detected in generated code:

```toml
[conditional]
# Include rust_decimal if rust_decimal::Decimal is detected
rust_decimal = { detect = "rust_decimal::Decimal", workspace = true }

# Include rusty-money if rusty_money::Money is detected
rusty-money = { detect = "rusty_money::Money", workspace = true, features = ["serde"] }

# Include chrono if any chrono type is detected (partial match)
chrono = { detect = "chrono::", workspace = true }

# Conditional with version
my-decimal = { detect = "my_decimal::Decimal", version = "2.0", features = ["serde"] }
```

## Example: Invoice Service

```toml
# openapi/accounting/invoice/brrtrouter-dependencies.toml

[dependencies]
# Always include these
serde_with = { workspace = true, features = ["chrono"] }

[conditional]
# Include if Decimal types are detected
rust_decimal = { detect = "rust_decimal::Decimal", workspace = true }
```

## Example: Budget Service

```toml
# openapi/accounting/budget/brrtrouter-dependencies.toml

[dependencies]
# Budget-specific dependencies
budget-calculator = { workspace = true }

[conditional]
# Include if Money types are detected
rusty-money = { detect = "rusty_money::Money", workspace = true, features = ["serde"] }
```

## How It Works

1. **Config Loading**: BRRTRouter looks for `brrtrouter-dependencies.toml` alongside the OpenAPI spec
2. **Type Detection**: Scans generated code for type patterns specified in `[conditional]`
3. **Dependency Inclusion**: 
   - Always includes `[dependencies]` entries
   - Conditionally includes `[conditional]` entries if types are detected
4. **Template Rendering**: Adds dependencies to generated `Cargo.toml`

## Integration with DependencyRegistry

The config file works alongside the built-in `DependencyRegistry`:
- **DependencyRegistry**: Handles common cases (rust_decimal, rusty-money, chrono, uuid)
- **Config File**: Handles service-specific or custom dependencies

Both systems work together - you can use either or both.

## Benefits

✅ **Per-Service Configuration**: Each microservice can have its own dependencies  
✅ **Version Controlled**: Config file is version controlled with your OpenAPI spec  
✅ **Auto-Detected**: No CLI flags needed (unless overriding)  
✅ **Type-Aware**: Conditional dependencies only included when types are actually used  
✅ **Flexible**: Supports workspace, version, path, and git dependencies  

## Migration from Hardcoded Dependencies

If you were previously hardcoding dependencies in BRRTRouter templates:

1. **Create config file** alongside your OpenAPI spec
2. **Move dependencies** to the config file
3. **Remove hardcoding** from BRRTRouter (future cleanup)

This keeps BRRTRouter generic while allowing per-service customization.
