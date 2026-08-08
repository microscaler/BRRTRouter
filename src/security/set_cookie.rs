//! `Set-Cookie` builder for **non-session** cookies (Epic 13.5 Option B).
//!
//! BRRTRouter does not provide server sessions or CSRF middleware. This helper
//! only formats RFC-compliant `Set-Cookie` header values with secure defaults
//! for app cookies (including JWT-in-cookie transport when the IdP issued the
//! token elsewhere).
//!
//! Defaults: `Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`.

use std::fmt::Write as _;

/// SameSite attribute for [`SetCookieBuilder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
    #[default]
    Lax,
    Strict,
    None,
}

impl SameSite {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lax => "Lax",
            Self::Strict => "Strict",
            Self::None => "None",
        }
    }
}

/// Builder for a single `Set-Cookie` header value.
///
/// Never panics on empty name/value — empty name yields an empty string (caller
/// should treat as invalid). Cookie values are not logged by this type (NFR-1).
#[derive(Debug, Clone)]
pub struct SetCookieBuilder {
    name: String,
    value: String,
    path: String,
    domain: Option<String>,
    max_age: Option<u64>,
    secure: bool,
    http_only: bool,
    same_site: SameSite,
}

impl SetCookieBuilder {
    /// Start a builder with production-oriented defaults (`Secure` + `HttpOnly` + `SameSite=Lax`).
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            path: "/".into(),
            domain: None,
            max_age: None,
            secure: true,
            http_only: true,
            same_site: SameSite::Lax,
        }
    }

    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    #[must_use]
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    #[must_use]
    pub fn max_age(mut self, secs: u64) -> Self {
        self.max_age = Some(secs);
        self
    }

    /// Clear cookie (`Max-Age=0`).
    #[must_use]
    pub fn clear(mut self) -> Self {
        self.max_age = Some(0);
        self.value = String::new();
        self
    }

    #[must_use]
    pub fn secure(mut self, on: bool) -> Self {
        self.secure = on;
        self
    }

    #[must_use]
    pub fn http_only(mut self, on: bool) -> Self {
        self.http_only = on;
        self
    }

    #[must_use]
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    /// Dev-only convenience: `Secure=false` (never use in production profiles).
    #[must_use]
    pub fn insecure_dev(mut self) -> Self {
        self.secure = false;
        self
    }

    /// Format the `Set-Cookie` header value (no header name prefix).
    #[must_use]
    pub fn build(&self) -> String {
        if self.name.is_empty() {
            return String::new();
        }
        // Sanitize: strip CR/LF from name/value to avoid header injection (no panic).
        let name = strip_ctl(&self.name);
        let value = strip_ctl(&self.value);
        if name.is_empty() {
            return String::new();
        }
        let mut out = String::with_capacity(64 + name.len() + value.len());
        let _ = write!(&mut out, "{name}={value}");
        let path = strip_ctl(&self.path);
        if !path.is_empty() {
            let _ = write!(&mut out, "; Path={path}");
        }
        if let Some(d) = &self.domain {
            let d = strip_ctl(d);
            if !d.is_empty() {
                let _ = write!(&mut out, "; Domain={d}");
            }
        }
        if let Some(age) = self.max_age {
            let _ = write!(&mut out, "; Max-Age={age}");
        }
        if self.http_only {
            out.push_str("; HttpOnly");
        }
        if self.secure {
            out.push_str("; Secure");
        }
        // SameSite=None requires Secure per modern browsers; still emit as configured.
        let _ = write!(&mut out, "; SameSite={}", self.same_site.as_str());
        out
    }
}

fn strip_ctl(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\r' && *c != '\n' && *c != '\0')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2a_defaults_httponly_secure_samesite() {
        let v = SetCookieBuilder::new("token", "abc").build();
        assert!(v.contains("HttpOnly"), "{v}");
        assert!(v.contains("Secure"), "{v}");
        assert!(v.contains("SameSite=Lax"), "{v}");
        assert!(v.starts_with("token=abc"), "{v}");
        assert!(v.contains("Path=/"), "{v}");
    }

    #[test]
    fn n5_empty_name_no_panic() {
        let v = SetCookieBuilder::new("", "x").build();
        assert!(v.is_empty());
    }

    #[test]
    fn n5_crlf_stripped_no_panic() {
        let v = SetCookieBuilder::new("a\r\nb", "c\nd").build();
        assert!(!v.contains('\r'));
        assert!(!v.contains('\n'));
        assert!(v.starts_with("ab=cd") || v.starts_with("ab="));
    }

    #[test]
    fn max_age_and_clear() {
        let v = SetCookieBuilder::new("x", "1").max_age(3600).build();
        assert!(v.contains("Max-Age=3600"));
        let c = SetCookieBuilder::new("x", "1").clear().build();
        assert!(c.contains("Max-Age=0"));
    }
}
