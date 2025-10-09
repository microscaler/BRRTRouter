//! # Typed Module
//!
//! The typed module provides type-safe request and response handling for BRRTRouter handlers.
//! It enables automatic deserialization of request data into strongly-typed Rust structs
//! and serialization of responses.
//!
//! ## Overview
//!
//! Instead of working with raw `HandlerRequest` and `HandlerResponse` types, handlers can
//! use typed structs that automatically convert to/from the wire format. This provides:
//!
//! - **Type Safety** - Compile-time checking of request/response structures
//! - **Automatic Conversion** - Derive macros handle serialization/deserialization
//! - **Better IDE Support** - Full autocomplete and type checking in handlers
//! - **Validation** - Type constraints enforce valid data at the type level
//!
//! ## Usage
//!
//! Define typed request and response structs:
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct GetPetRequest {
//!     pet_id: String,
//! }
//!
//! #[derive(Serialize)]
//! struct GetPetResponse {
//!     id: String,
//!     name: String,
//!     species: String,
//! }
//! ```
//!
//! Use them in handlers with automatic conversion:
//!
//! ```rust,ignore
//! fn get_pet(req: GetPetRequest) -> GetPetResponse {
//!     GetPetResponse {
//!         id: req.pet_id.clone(),
//!         name: "Fluffy".to_string(),
//!         species: "Cat".to_string(),
//!     }
//! }
//! ```
//!
//! ## Code Generation
//!
//! The BRRTRouter generator automatically creates typed request/response structs
//! from your OpenAPI schemas, so you get type-safe handlers without manual struct definitions.
//!
//! ## Benefits
//!
//! Typed handlers provide several advantages over raw handlers:
//!
//! - No manual JSON parsing or serialization
//! - Compile-time verification of field names and types
//! - Automatic parameter extraction from path/query/body
//! - Built-in validation through type constraints
//! - Cleaner, more readable handler code

mod core;

pub use core::*;
