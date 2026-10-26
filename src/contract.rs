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
//! contract.assert_strict("get_me", 200, &body); // and no key the schema does not declare
//! ```

use std::collections::HashMap;

use serde_json::Value;

use crate::spec::RouteMeta;

/// At most this many schema errors are reported per check (the server logs the same number).
const MAX_ERRORS: usize = 20;

/// Why a value does not meet an operation's response contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// No operation with that handler name in the spec (see [`ResponseContract::check`]).
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
            Self::UnknownOperation(op) => {
                write!(f, "no operation with handler name `{op}` in the spec")
            }
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
    /// `operation` is the handler name the server dispatches to: the operation's `x-handler-*`
    /// extension when it has one, else its `operationId` (as [`crate::spec::RouteMeta`]'s
    /// `handler_name`, and as the generated handler is named).
    ///
    /// # Errors
    ///
    /// See [`ContractError`].
    pub fn check(&self, operation: &str, status: u16, body: &Value) -> Result<(), ContractError> {
        let schema = self.schema(operation, status)?;
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

    /// Keys in `body` that the response schema does not declare, as JSON pointers.
    ///
    /// A schema without `additionalProperties: false` lets any extra key through, so
    /// [`check`](Self::check) passes a body that sends `phone` where the spec says
    /// `phone_number`. This walks the body beside the schema: at each object whose schema has
    /// `properties`, a key not among them is reported; it recurses into declared properties and
    /// array `items`. A node whose schema combines others (`allOf`, `anyOf`, `oneOf`) or allows
    /// any keys explicitly (`additionalProperties: true` or a schema) is not inspected.
    ///
    /// # Errors
    ///
    /// [`ContractError::UnknownOperation`] or [`ContractError::NoSchema`].
    pub fn undeclared_fields(
        &self,
        operation: &str,
        status: u16,
        body: &Value,
    ) -> Result<Vec<String>, ContractError> {
        let schema = self.schema(operation, status)?;
        let mut out = Vec::new();
        collect_undeclared(schema, body, &mut String::new(), &mut out);
        Ok(out)
    }

    /// [`check`](Self::check) plus [`undeclared_fields`](Self::undeclared_fields) empty,
    /// panicking with what is wrong: for `#[test]`s that hold a hand-built body to the spec's
    /// field names as well as its types.
    ///
    /// # Panics
    ///
    /// When either check fails.
    #[track_caller]
    pub fn assert_strict(&self, operation: &str, status: u16, body: &Value) {
        self.assert_valid(operation, status, body);
        match self.undeclared_fields(operation, status, body) {
            Ok(extra) if extra.is_empty() => {}
            Ok(extra) => panic!(
                "`{operation}` {status} response sends fields its schema does not declare: {}",
                extra.join(", ")
            ),
            Err(e) => panic!("{e}"),
        }
    }

    fn schema(&self, operation: &str, status: u16) -> Result<&Value, ContractError> {
        let route = self
            .routes
            .get(operation)
            .ok_or_else(|| ContractError::UnknownOperation(operation.to_string()))?;
        crate::server::service::response_body_schema_for_status(route, status).ok_or_else(|| {
            ContractError::NoSchema {
                operation: operation.to_string(),
                status,
            }
        })
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

fn collect_undeclared(schema: &Value, value: &Value, path: &mut String, out: &mut Vec<String>) {
    if ["allOf", "anyOf", "oneOf"]
        .iter()
        .any(|k| schema.get(*k).is_some())
    {
        return;
    }
    match value {
        Value::Object(map) => {
            let Some(props) = schema.get("properties").and_then(Value::as_object) else {
                return;
            };
            let open = matches!(
                schema.get("additionalProperties"),
                Some(Value::Bool(true) | Value::Object(_))
            );
            for (key, v) in map {
                let len = path.len();
                path.push('/');
                path.push_str(&key.replace('~', "~0").replace('/', "~1"));
                match props.get(key) {
                    Some(sub) => collect_undeclared(sub, v, path, out),
                    None if !open => out.push(path.clone()),
                    None => {}
                }
                path.truncate(len);
            }
        }
        Value::Array(items) => {
            let Some(item_schema) = schema.get("items") else {
                return;
            };
            for (i, v) in items.iter().enumerate() {
                let len = path.len();
                path.push('/');
                path.push_str(&i.to_string());
                collect_undeclared(item_schema, v, path, out);
                path.truncate(len);
            }
        }
        _ => {}
    }
}
