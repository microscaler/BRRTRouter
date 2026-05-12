//! # BRRTRouter Tooling
//!
//! Rust library for BRRTRouter project orchestration. Replaces the Python tooling
//! with native Rust implementations for:
//! - Code generation from OpenAPI specs (via brrtrouter-gen)
//! - Microservice workspace builds (host-aware, multi-arch)
//! - Docker artifact copying and validation
//! - Suite/service discovery and port management
//!
//! ## Module organization
//!
//! - `paths` — BRRTRouter root discovery (env var, parent-relative)
//! - `discovery` — Suite, service, BFF, and port metadata from filesystem
//! - `gen` — Service code generation via brrtrouter-gen
//! - `build` — Build orchestration (cargo/cross/zigbuild)
//! - `docker` — Artifact copying and validation
//! - `ci` — Post-gen fixes (Cargo.toml path rewriting)

pub mod build;
pub mod ci;
pub mod discovery;
pub mod docker;
pub mod gen;
pub mod paths;

/// Result type alias for tooling operations
pub type ToolingResult<T = ()> = anyhow::Result<T>;
