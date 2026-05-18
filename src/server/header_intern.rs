//! Zero-allocation intern table for common HTTP header names (PRD Phase 2.1).
//!
//! Before this module, [`parse_request`](super::request::parse_request) did two
//! allocations **per header** on **every request**: one `String` from
//! `h.name.to_ascii_lowercase()` and a second `Arc<str>` from `Arc::from(...)`.
//! With ~10 headers/request at ~60 k req/s sustained that is 1.2 M allocations
//! per second purely for header naming — measurable in flamegraphs and a
//! significant fraction of the request hot-path cost.
//!
//! This module pre-allocates `Arc<str>` copies of the canonical lowercased form
//! of the ~24 most common HTTP header names at first use. A case-insensitive
//! byte-compare against the table returns an O(1) `Arc::clone` on hit — no
//! heap allocation. On miss (unusual or custom header names) we fall back to
//! the original two-alloc path so behaviour is otherwise unchanged.
//!
//! The table is intentionally small and linear — at N ≈ 24 the per-lookup
//! cost is a handful of pointer-compares + short byte-wise compares, cheaper
//! than either hashing or two heap allocations.

use once_cell::sync::Lazy;
use std::sync::Arc;

/// Canonical lowercased names that we expect to see repeatedly across traffic.
/// Ordered roughly by observed frequency (content-type and host dominate).
const COMMON_HEADER_NAMES: &[&str] = &[
    "content-type",
    "content-length",
    "content-encoding",
    "host",
    "user-agent",
    "connection",
    "accept",
    "accept-encoding",
    "accept-language",
    "authorization",
    "cookie",
    "referer",
    "origin",
    "x-api-key",
    "x-request-id",
    "x-forwarded-for",
    "x-forwarded-proto",
    "x-real-ip",
    "if-none-match",
    "if-match",
    "etag",
    "date",
    "cache-control",
    "upgrade",
];

/// Static intern table: `(lowercased bytes, canonical Arc<str>)`. The `Arc`s
/// live for the lifetime of the process; `Arc::clone` on a hit is a single
/// relaxed atomic fetch_add.
static INTERNED: Lazy<Vec<(&'static [u8], Arc<str>)>> = Lazy::new(|| {
    COMMON_HEADER_NAMES
        .iter()
        .map(|n| (n.as_bytes(), Arc::from(*n)))
        .collect()
});

/// Intern `raw_name` to an `Arc<str>` holding the **lowercase** form of the
/// name.
///
/// * Hit (~95 %+ of real HTTP traffic): returns a shared `Arc` with one
///   atomic refcount bump. Zero heap allocation.
/// * Miss: falls back to `Arc::from(lowercased)` — identical behaviour to
///   the pre-intern path, so any custom header still works.
///
/// Case-insensitive ASCII comparison, matching RFC 9110 §5.1.
#[inline]
pub fn intern_header_name(raw_name: &[u8]) -> Arc<str> {
    for (lower_bytes, arc) in INTERNED.iter() {
        if raw_name.len() == lower_bytes.len() && eq_ignore_ascii_case_bytes(raw_name, lower_bytes)
        {
            return Arc::clone(arc);
        }
    }
    // Miss — allocate lowercased owned string, then Arc<str>. This mirrors the
    // pre-Phase-2.1 behaviour for any non-standard header name.
    let mut lower = Vec::with_capacity(raw_name.len());
    lower.extend(raw_name.iter().map(|b| b.to_ascii_lowercase()));
    // SAFETY: `raw_name` was a valid UTF-8 header name (httparse rejects non-
    // ASCII names) and `to_ascii_lowercase` only touches ASCII letters, so the
    // result remains valid UTF-8.
    let lower_str = unsafe { std::str::from_utf8_unchecked(&lower) };
    Arc::from(lower_str)
}

/// Inline byte-wise ASCII case-insensitive compare. `a` is the incoming raw
/// header (may be any case); `b_lower` is known to be lowercase already.
#[inline]
fn eq_ignore_ascii_case_bytes(a: &[u8], b_lower: &[u8]) -> bool {
    debug_assert_eq!(a.len(), b_lower.len());
    for (x, y) in a.iter().zip(b_lower.iter()) {
        // Fast path: exact byte match (covers the dominant case where clients
        // send lowercased names, e.g. HTTP/2 peers, modern proxies).
        if x == y {
            continue;
        }
        if x.to_ascii_lowercase() != *y {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_hit_returns_shared_arc() {
        let a = intern_header_name(b"content-type");
        let b = intern_header_name(b"Content-Type");
        let c = intern_header_name(b"CONTENT-TYPE");
        assert_eq!(&*a, "content-type");
        assert_eq!(&*b, "content-type");
        assert_eq!(&*c, "content-type");
        // All three are the same underlying Arc (hit path), so strong_count
        // bumps rather than producing new allocations.
        assert!(Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&b, &c));
    }

    #[test]
    fn intern_miss_allocates_lowercase() {
        let a = intern_header_name(b"X-Custom-Weird-Header");
        assert_eq!(&*a, "x-custom-weird-header");
        let b = intern_header_name(b"X-Custom-Weird-Header");
        // Miss path: new Arc each time — but values are equal.
        assert_eq!(&*a, &*b);
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn intern_handles_every_canonical_name() {
        for name in COMMON_HEADER_NAMES {
            let arc = intern_header_name(name.as_bytes());
            assert_eq!(&*arc, *name);
        }
    }

    #[test]
    fn intern_length_guard_rejects_prefix_matches() {
        // A raw name that is a prefix of a canonical entry must not collide.
        let arc = intern_header_name(b"content");
        assert_eq!(&*arc, "content");
        // And must not alias the canonical Arc for `content-type`.
        let ct = intern_header_name(b"content-type");
        assert!(!Arc::ptr_eq(&arc, &ct));
    }
}
