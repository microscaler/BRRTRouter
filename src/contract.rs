//! Response contract checks for tests.
//!
//! A controller that builds its JSON by hand (`serde_json::json!`, `HttpJson<Value>`) has no
//! generated type to keep it in line with the spec; at runtime only response validation notices
//! a drift, as a 500. [`ResponseContract`] runs the same check in a unit test: load the spec once,
//! then check a value against an operation's response schema for a status.
//!
//! The schema is chosen as the server chooses it ([`crate::server`]'s response validation):
//! the status's `application/json` schema, else another JSON-ish one, else (2xx) the
//! operation's default success schema.
//!
//! ```rust,ignore
//! let contract = brrtrouter::contract::ResponseContract::load("doc/openapi.yaml")?;
//! contract.assert_valid("get_me", 200, &body);
//! ```

use std::collections::HashMap;

use serde_json::Value;

use crate::spec::RouteMeta;

/// At most this many schema errors are reported per check (the server logs the same number).
const MAX_ERRORS: usize = 20;

/// Why a value does not meet an operation's response contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// No operation with that `operationId` (or `x-handler-*` name) in the spec.
    UnknownOperation(String),
    /// The operation declares no JSON schema for this status.
    NoSchema { operation: String, status: u16 },
    /// The schema itself does not compile.
    BadSchema {
        operation: String,
        status: u16,
        error: String,
    },
    /// The value does not match the schema; each entry is `"<instance path>: <message>"`.
    Mismatch {
        operation: String,
        status: u16,
        errors: Vec<String>,
    },
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownOperation(op) => write!(f, "no operation `{op}` in the spec"),
            Self::NoSchema { operation, status } => {
                write!(f, "`{operation}` declares no JSON schema for {status}")
            }
            Self::BadSchema {
                operation,
                status,
                error,
            } => {
                write!(f, "`{operation}` {status} schema does not compile: {error}")
            }
            Self::Mismatch {
                operation,
                status,
                errors,
            } => {
                write!(
                    f,
                    "`{operation}` {status} response does not match its schema:"
                )?;
                for e in errors {
                    write!(f, "\n  - {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ContractError {}

/// The response schemas of one spec, by operation.
pub struct ResponseContract {
    routes: HashMap<String, RouteMeta>,
}

impl ResponseContract {
    /// Load the spec at `spec_path` (YAML or JSON) as the server would.
    ///
    /// # Errors
    ///
    /// The spec cannot be read or parsed.
    pub fn load(spec_path: &str) -> anyhow::Result<Self> {
        let (routes, _slug) = crate::spec::load_spec(spec_path)?;
        Ok(Self::from_routes(routes))
    }

    /// Build from routes already loaded.
    #[must_use]
    pub fn from_routes(routes: Vec<RouteMeta>) -> Self {
        Self {
            routes: routes
                .into_iter()
                .map(|r| (r.handler_name.to_string(), r))
                .collect(),
        }
    }

    /// Check `body` against `operation`'s response schema for `status`.
    ///
    /// # Errors
    ///
    /// See [`ContractError`].
    pub fn check(&self, operation: &str, status: u16, body: &Value) -> Result<(), ContractError> {
        let route = self
            .routes
            .get(operation)
            .ok_or_else(|| ContractError::UnknownOperation(operation.to_string()))?;
        let schema = crate::server::service::response_body_schema_for_status(route, status)
            .ok_or_else(|| ContractError::NoSchema {
                operation: operation.to_string(),
                status,
            })?;
        let validator =
            jsonschema::validator_for(schema).map_err(|e| ContractError::BadSchema {
                operation: operation.to_string(),
                status,
                error: e.to_string(),
            })?;
        if validator.is_valid(body) {
            return Ok(());
        }
        let errors = validator
            .iter_errors(body)
            .take(MAX_ERRORS)
            .map(|e| {
                let path = e.instance_path().to_string();
                if path.is_empty() {
                    e.to_string()
                } else {
                    format!("{path}: {e}")
                }
            })
            .collect();
        Err(ContractError::Mismatch {
            operation: operation.to_string(),
            status,
            errors,
        })
    }

    /// [`check`](Self::check), panicking with the errors listed: for `#[test]`s.
    ///
    /// # Panics
    ///
    /// When the check fails.
    #[track_caller]
    pub fn assert_valid(&self, operation: &str, status: u16, body: &Value) {
        if let Err(e) = self.check(operation, status, body) {
            panic!("{e}");
        }
    }

    /// Every operation name in the spec, sorted (to check each has a test).
    #[must_use]
    pub fn operations(&self) -> Vec<&str> {
        let mut ops: Vec<&str> = self.routes.keys().map(String::as_str).collect();
        ops.sort_unstable();
        ops
    }
}

/// Check `body` against one operation's response schema in the spec at `spec_path`.
/// Loading the spec each call is slow; for several checks keep a [`ResponseContract`].
///
/// # Errors
///
/// The spec does not load, or see [`ContractError`].
pub fn check_response(
    spec_path: &str,
    operation: &str,
    status: u16,
    body: &Value,
) -> anyhow::Result<()> {
    ResponseContract::load(spec_path)?
        .check(operation, status, body)
        .map_err(Into::into)
}
