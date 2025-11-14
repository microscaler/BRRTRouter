//! # CLI Module
//!
//! The CLI module provides command-line interface functionality for the BRRTRouter
//! code generator and utilities.
//!
//! ## Overview
//!
//! The CLI supports:
//! - **Code Generation** - Generate complete services from OpenAPI specifications
//! - **Validation** - Validate OpenAPI specs for correctness
//! - **Introspection** - Inspect routes and handlers from specs
//!
//! ## Commands
//!
//! ### `generate`
//!
//! Generate a complete service from an OpenAPI specification:
//!
//! ```bash
//! brrtrouter-gen generate --spec openapi.yaml --output my-service
//! ```
//!
//! Options:
//! - `--spec <FILE>` - Path to OpenAPI specification (required)
//! - `--output <DIR>` - Output directory for generated project (required)
//! - `--force` - Overwrite existing files without prompting
//! - `--scope <SCOPE>` - Generation scope: full, handlers, controllers (default: full)
//!
//! ### `validate`
//!
//! Validate an OpenAPI specification:
//!
//! ```bash
//! brrtrouter-gen validate --spec openapi.yaml
//! ```
//!
//! ### `inspect`
//!
//! Inspect routes and handlers in a specification:
//!
//! ```bash
//! brrtrouter-gen inspect --spec openapi.yaml
//! ```
//!
//! ## Usage from Code
//!
//! ```rust,ignore
//! use brrtrouter::cli::{Cli, Commands, run_cli};
//! use clap::Parser;
//!
//! let cli = Cli::parse();
//! run_cli(cli)?;
//! ```
//!
//! ## Binary
//!
//! The CLI is available as the `brrtrouter-gen` binary:
//!
//! ```bash
//! cargo install brrtrouter
//! brrtrouter-gen --help
//! ```
//!
//! ## Examples
//!
//! ```bash
//! # Generate a new service
//! brrtrouter-gen generate \
//!     --spec examples/openapi.yaml \
//!     --output my-api
//!
//! # Regenerate only handlers
//! brrtrouter-gen generate \
//!     --spec openapi.yaml \
//!     --output my-api \
//!     --scope handlers \
//!     --force
//!
//! # Validate a spec before generation
//! brrtrouter-gen validate --spec openapi.yaml
//! ```

mod commands;

#[cfg(test)]
mod tests;

pub use commands::{run_cli, Cli, Commands};
