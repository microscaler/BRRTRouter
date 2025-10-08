# BRRTRouter Documentation

This document provides an overview of BRRTRouter's documentation structure and guidance for contributors.

## Documentation Status

### ✅ Completed

- **Module-level documentation** - All public modules now have comprehensive `//!` documentation
- **Architecture documentation** - Each module explains its design and integration points
- **Usage examples** - Practical examples included in all module docs
- **CI integration** - Documentation linting added to GitHub Actions workflow
- **Contributing guide** - Documentation standards added to CONTRIBUTING.md

### 📋 Remaining Tasks

The following documentation tasks are recommended for future work:

1. **Function-level documentation** - Add `///` doc comments to all public functions, structs, and traits
2. **Complex function examples** - Add detailed examples to non-trivial functions
3. **Test documentation** - Document test modules with coverage and purpose
4. **API guides** - Create user guides for common scenarios
5. **Architecture diagrams** - Add visual diagrams to complex modules

## Documentation Structure

### Module Documentation

Each module includes:

- **Title and overview** - What the module does
- **Architecture** - How it works internally
- **Usage examples** - Practical code examples
- **Key types** - Links to important exported types
- **Performance notes** - Performance considerations where relevant

### Documented Modules

| Module | Description | Status |
|--------|-------------|--------|
| `lib.rs` | Library root with quick start guide | ✅ Complete |
| `router` | Path matching and route resolution | ✅ Complete |
| `dispatcher` | Coroutine-based request dispatch | ✅ Complete |
| `spec` | OpenAPI 3.1 parsing and loading | ✅ Complete |
| `server` | HTTP server implementation | ✅ Complete |
| `middleware` | Middleware system (auth, CORS, metrics) | ✅ Complete |
| `security` | Authentication and authorization | ✅ Complete |
| `generator` | Code generation from OpenAPI specs | ✅ Complete |
| `typed` | Type-safe request/response handling | ✅ Complete |
| `validator` | OpenAPI spec validation | ✅ Complete |
| `hot_reload` | Live spec reloading | ✅ Complete |
| `sse` | Server-Sent Events support | ✅ Complete |
| `static_files` | Static file serving with templates | ✅ Complete |
| `runtime_config` | Environment-based configuration | ✅ Complete |
| `cli` | Command-line interface | ✅ Complete |

## Viewing Documentation

### Locally

Generate and open documentation in your browser:

```bash
cargo doc --no-deps --lib --open
```

### Online

Once published to crates.io, documentation will be available at:
- https://docs.rs/brrtrouter

## Documentation Guidelines

### For Module Documentation (`//!`)

1. Start with a clear title: `//! # Module Name`
2. Provide an overview of the module's purpose
3. Explain the architecture and design decisions
4. Include at least one usage example
5. Link to key types using `[`TypeName`]` syntax

### For Item Documentation (`///`)

1. Start with a brief description (one line)
2. Add detailed explanation if needed
3. Document all parameters using `# Arguments`
4. Document return values using `# Returns`
5. Include examples for non-trivial functions
6. Document panic conditions with `# Panics`
7. Document safety requirements with `# Safety` (for unsafe code)

### Example Template

```rust
/// Brief one-line description of the function.
///
/// Longer explanation of what this function does, when to use it,
/// and any important details about its behavior.
///
/// # Arguments
///
/// * `param1` - Description of first parameter
/// * `param2` - Description of second parameter
///
/// # Returns
///
/// Returns X on success, or Y if Z condition occurs.
///
/// # Examples
///
/// ```rust
/// use brrtrouter::module::function;
///
/// let result = function(arg1, arg2)?;
/// assert_eq!(result, expected);
/// ```
///
/// # Panics
///
/// Panics if the parameter is invalid.
pub fn function(param1: Type1, param2: Type2) -> Result<Return, Error> {
    // Implementation
}
```

## Testing Documentation

### Compile Documentation

Check that all documentation compiles:

```bash
RUSTDOCFLAGS="-D warnings -D rustdoc::broken_intra_doc_links" cargo doc --no-deps --lib
```

### Test Documentation Examples

Run documentation tests:

```bash
cargo test --doc
```

## CI Integration

The GitHub Actions workflow automatically:

1. Generates documentation
2. Checks for warnings
3. Checks for broken intra-doc links
4. Verifies documentation examples compile

See `.github/workflows/ci.yml` for details.

## Contributing

When adding new public APIs:

1. Add module-level docs if creating a new module
2. Add doc comments to all public items
3. Include at least one example
4. Link to related types using `[`TypeName`]`
5. Run `cargo doc` to verify rendering
6. Run `cargo fmt` before committing

For more details, see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Architecture Documentation

For detailed architectural information, see:

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Comprehensive architecture guide with sequence diagrams
  - Code generation flow
  - Request handling flow
  - Key components and patterns
  - Extension points

## Resources

- [Rust Documentation Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html)
- [Rustdoc Book](https://doc.rust-lang.org/rustdoc/)
- [Writing Documentation Comments](https://doc.rust-lang.org/book/ch14-02-publishing-to-crates-io.html#making-useful-documentation-comments)

## Questions?

If you have questions about documentation:

1. Check this guide first
2. Look at existing documented modules for examples
3. Review the Rust documentation guidelines
4. Ask in a GitHub issue or pull request

