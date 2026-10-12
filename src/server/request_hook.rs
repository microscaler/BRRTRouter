//! An application hook that sees every request before routing and security.
//!
//! The case it exists for: a backend-for-frontend (BFF) whose browser sends an
//! opaque session cookie, while its routes are secured with `BearerAuth` and its
//! proxied backends expect `Authorization: Bearer`. The hook resolves the cookie
//! to the session's access token and sets the header, so route security and the
//! proxy see an ordinary bearer request. It can also answer the request itself
//! (a CSRF refusal, a session store outage), and the route never runs.
//!
//! The hook runs after the built-in endpoints (`/health`, `/ready`, `/metrics`,
//! docs) and before static files, routing, CORS preflight handling and
//! security. A `Cookie` header it changes is reparsed, so security and
//! handlers see the same cookies the hook left. Its answer follows the same
//! rules as a handler's: no body on `HEAD`, and a live SSE response
//! ([`HandlerResponse::sse`]) is streamed.

use crate::dispatcher::{HandlerResponse, HeaderVec};
use crate::router::ParamVec;
use http::Method;
use std::sync::Arc;

/// What a [`RequestHook`] can see and change.
pub struct RequestHookContext<'a> {
    pub method: &'a Method,
    pub path: &'a str,
    /// Request headers. Names are matched case-insensitively.
    pub headers: &'a mut HeaderVec,
    /// Parsed `Cookie` header pairs.
    pub cookies: &'a HeaderVec,
    /// Decoded query parameters.
    pub query: &'a ParamVec,
}

impl RequestHookContext<'_> {
    /// A request header by name (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The raw `Cookie` header, if any.
    #[must_use]
    pub fn cookie_header(&self) -> Option<&str> {
        self.header("cookie")
    }

    /// A cookie by name.
    #[must_use]
    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// A query parameter by name (the first, if repeated).
    #[must_use]
    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Replace every header called `name` (case-insensitive) with one `value`.
    pub fn set_header(&mut self, name: &str, value: impl Into<String>) {
        self.remove_header(name);
        // Interned: a hook sets the same few names (authorization) on every request.
        self.headers.push((
            super::header_intern::intern_header_name(name.as_bytes()),
            value.into(),
        ));
    }

    /// Remove every header called `name` (case-insensitive).
    pub fn remove_header(&mut self, name: &str) {
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
    }
}

/// Runs before routing and security. `None` lets the request continue (with any
/// header changes); `Some(response)` answers it at once.
pub type RequestHook =
    Arc<dyn for<'a> Fn(&mut RequestHookContext<'a>) -> Option<HandlerResponse> + Send + Sync>;
