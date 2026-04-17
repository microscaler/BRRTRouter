# Bug Report: Thread-Local Context Corruption on macOS with Rust 1.90.0

## Status: ✅ FIXED

Our fork of generator-rs at `microscaler/generator-rs` branch `fix/rust-1.90-thread-local-macos` fixes this issue.

## Summary

`generator-rs` v0.8.7 crashes in release builds on macOS with Rust 1.90.0-nightly due to thread-local storage optimization issues. The existing workaround for Rust 1.89 (calling `std::thread::current()` twice) was insufficient for newer Rust versions.

## Environment

- **OS**: macOS 15.1 (Darwin 25.1.0)
- **Architecture**: aarch64 (Apple Silicon)
- **Rust**: 1.90.0-nightly (f26e58023 2025-06-30)
- **generator-rs**: 0.8.7
- **may**: 0.3.51

## Symptoms

Under heavy load, may-based servers crash with:

```
thread '<unnamed>' panicked at may-0.3.51/src/coroutine_impl.rs:477:17:
no cancel data, did you call `current_cancel_data()` in thread context?
```

Or:

```
thread '<unnamed>' panicked at generator-0.8.7/src/yield_.rs:127:32:
called `Option::unwrap()` on a `None` value
```

## Root Cause

The thread-local `ROOT_CONTEXT_P` value in `src/rt.rs` is being incorrectly optimized by the Rust 1.90 compiler in release builds, causing `get_local_data()` to return null even when inside a valid coroutine context.

## The Fix

Our fix applies multiple strategies to prevent the optimization:

1. **`#[inline(never)]`** on critical functions in release mode on macOS/Windows to prevent problematic inlining

2. **`compiler_fence(Ordering::SeqCst)`** before and after TLS access to prevent instruction reordering

3. **`std::hint::black_box()`** to prevent the compiler from optimizing away or caching TLS values

### Fixed Functions

- `ContextStack::current()`
- `ContextStack::co_ctx()`
- `is_generator()`
- `get_local_data()`

## Test Results

| Configuration | Requests | Errors | Status |
|--------------|----------|--------|--------|
| Unpatched (nightly release) | ~200k | Crash | ❌ |
| Debug build (nightly) | 1.4M | 0 | ✅ |
| Release build (stable 1.86) | 2.7M | 0 | ✅ |
| **Patched (nightly release)** | **5.5M+** | **0** | ✅ |

## Usage

Add to your `Cargo.toml`:

```toml
[patch.crates-io]
generator = { git = "https://github.com/microscaler/generator-rs.git", branch = "fix/rust-1.90-thread-local-macos" }
```

## Related

- Fork: https://github.com/microscaler/generator-rs
- Branch: `fix/rust-1.90-thread-local-macos`
- Upstream: https://github.com/Xudong-Huang/generator-rs

## Action Required: Create Upstream PR

**PR Title**: `fix: prevent thread-local optimization issues on macOS/Windows with Rust 1.90+`

**Target**: `Xudong-Huang/generator-rs` master branch

**Source**: `microscaler/generator-rs` branch `fix/rust-1.90-thread-local-macos`

**URL to create PR**: https://github.com/Xudong-Huang/generator-rs/compare/master...microscaler:generator-rs:fix/rust-1.90-thread-local-macos?expand=1

## Validation Complete

- ✅ Local testing: 5.5M+ requests, 0 errors
- ✅ Kind cluster testing: Running stable
- ✅ Commits pushed to fork
