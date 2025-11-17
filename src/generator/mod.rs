//! # Generator Module
//!
//! The generator module provides code generation capabilities for BRRTRouter, automatically
//! creating complete service implementations from OpenAPI specifications.
//!
//! ## Overview
//!
//! The generator creates a complete, production-ready service from an OpenAPI spec, including:
//! - **Handlers** - Typed request/response handler stubs for each operation
//! - **Controllers** - Controller layer that calls handlers and builds responses
//! - **Type Definitions** - Rust structs generated from OpenAPI schemas
//! - **Main Binary** - Complete service with routing, middleware, and startup
//! - **Configuration** - YAML config files for security and HTTP settings
//! - **Documentation** - Generated HTML docs from the OpenAPI spec
//!
//! ## Architecture
//!
//! The generator uses Askama templates to produce Rust code:
//!
//! ```text
//! OpenAPI Spec → Parser → Schema Analysis → Template Rendering → Generated Code
//! ```
//!
//! 1. **Parser** - Loads and validates the OpenAPI specification
//! 2. **Schema Analysis** - Extracts types, operations, and dependencies
//! 3. **Template Rendering** - Renders Askama templates with extracted data
//! 4. **Code Generation** - Writes formatted Rust code to output directory
//!
//! ## Generated Structure
//!
//! A generated project has this structure:
//!
//! ```text
//! my-service/
//! ├── Cargo.toml              # Dependencies and project metadata
//! ├── config/
//! │   └── config.yaml         # Security and HTTP configuration
//! ├── doc/
//! │   ├── openapi.yaml        # Copy of the OpenAPI spec
//! │   └── openapi.html        # Rendered API documentation
//! ├── static_site/
//! │   └── index.html          # Landing page
//! └── src/
//!     ├── main.rs             # Service startup and configuration
//!     ├── registry.rs         # Handler registration
//!     ├── handlers/
//!     │   ├── mod.rs
//!     │   └── *.rs            # One file per operation
//!     └── controllers/
//!         ├── mod.rs
//!         └── *.rs            # One file per operation
//! ```
//!
//! ## Usage
//!
//! ### CLI Usage
//!
//! ```bash
//! cargo run --bin brrtrouter-gen -- generate \
//!     --spec openapi.yaml \
//!     --output my-service
//! ```
//!
//! ### Programmatic Usage
//!
//! ```rust,ignore
//! use brrtrouter::generator::{generate_project_from_spec, GenerationScope};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! generate_project_from_spec(
//!     "openapi.yaml",
//!     "my-service",
//!     GenerationScope::Full,
//!     false, // don't force overwrite
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Generation Scopes
//!
//! The generator supports different scopes:
//!
//! - **Full** - Generate complete project structure
//! - **HandlersOnly** - Regenerate only handler files
//! - **ControllersOnly** - Regenerate only controller files
//!
//! This allows iterative development where you modify handlers and regenerate controllers.
//!
//! ## Type Generation
//!
//! The generator analyzes OpenAPI schemas and generates Rust types:
//!
//! - **Object schemas** → `struct` definitions
//! - **Array schemas** → `Vec<T>` types
//! - **Enum schemas** → `enum` definitions
//! - **Primitives** → Rust primitive types
//! - **References** → Type aliases or imports
//!
//! ## Template Customization
//!
//! Templates are located in the `templates/` directory:
//!
//! - `handler.rs.txt` - Handler function template
//! - `controller.rs.txt` - Controller function template
//! - `main.rs.txt` - Main binary template
//! - `registry.rs.txt` - Handler registry template
//! - `Cargo.toml.txt` - Cargo manifest template
//!
//! Modify these templates to customize code generation.

mod project;
mod schema;
mod stack_size;
mod templates;
#[cfg(test)]
mod tests;

pub use project::*;
pub use schema::*;
pub use stack_size::*;
pub use templates::*;

pub use project::GenerationScope;
