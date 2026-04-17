# JSF AV Rules Application to BRRTRouter - Audit Opinion

**Date:** December 4, 2025  
**Auditor:** Claude (AI Assistant)  
**Documents Reviewed:**
- `JSF_WRITEUP.md` - Application of JSF AV rules to BRRTRouter
- `JSF-AV-rules.pdf` - Joint Strike Fighter Air Vehicle C++ Coding Standards

---

## Executive Summary

The `JSF_WRITEUP.md` document is an **exceptionally well-thought-out** adaptation of military-grade safety coding standards to a Rust-based HTTP router. The author has correctly identified which JSF principles translate meaningfully to Rust and which are already solved by the language itself.

**Overall Assessment: HIGHLY VALUABLE**

This document should be formalized into project standards. The proposed "BRRTRouter-SAFE" profile is practical, enforceable, and would meaningfully differentiate BRRTRouter from other Rust routers in latency-sensitive deployments.

---

## Analysis by Section

### 1. Bounded Complexity (JSF AV Rules 1, 3)

**Proposal:** Functions ≤80-100 lines, cyclomatic complexity <10-20 on hot path

**Opinion:** ✅ **STRONGLY AGREE**

This is practical and enforceable. The suggestion to use `cargo llvm-cov` with a CC threshold script is excellent. Current BRRTRouter hot path code (dispatcher, router) should be audited against this immediately.

**Recommendation:** Add a CI gate that fails if any function in `src/router/`, `src/dispatcher/` exceeds CC=15.

---

### 2. Allocation Discipline (JSF AV Rule 206)

**Proposal:** "No malloc after init" → "No heap in hot path"

**Opinion:** ✅ **STRONGLY AGREE - This is the crown jewel**

The document correctly identifies that Rust provides memory safety but NOT allocation determinism. The concrete rules:
- No `String::new`, `Vec::new`, `format!`, `to_string` in dispatch
- Route lookup must be index-based, not HashMap
- Use `SmallVec`, stack-allocated arrays, borrowed slices

This is **exactly right** for a high-performance router. The grep-based enforcement suggestion (`rg "format!|to_string|String::from"`) is crude but effective for CI.

**Current BRRTRouter Reality Check:** 
Looking at the codebase, there are allocations in the hot path. The radix trie proposal in the second half of the document would address this properly.

---

### 3. Error Handling (JSF AV Rule 208)

**Proposal:** No panics in hot path, no unwinding

**Opinion:** ✅ **STRONGLY AGREE**

The Clippy configuration provided is immediately actionable:
```rust
#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
```

BRRTRouter already has some panic protection with `catch_unwind` in handlers, but the router/dispatcher core should be hardened further.

---

### 4. Data & Type Rules (JSF AV Rules 148, 209, 215)

**Proposal:** Enums over integers, newtypes for IDs, minimal trait objects

**Opinion:** ✅ **AGREE with nuance**

The suggestion to wrap `RouteId` in a newtype and avoid `dyn Handler` in the innermost loop is sound. However, BRRTRouter's current architecture uses channels and trait objects for handler polymorphism - this is a fundamental design that works well for the May coroutine model.

**Recommendation:** Keep trait objects at handler boundaries but ensure route matching is monomorphic/enum-based.

---

### 5. Flow Control (JSF AV Rule 119)

**Proposal:** No recursion in router, structured branching

**Opinion:** ✅ **STRONGLY AGREE**

Non-recursive path matching is essential for predictable stack usage. The iterative radix trie implementation in the document demonstrates exactly how to achieve this.

---

### 6. Testing Discipline (JSF AV Rules 219-221)

**Proposal:** Cover all dynamic dispatch paths

**Opinion:** ✅ **AGREE**

The meta-test idea ("every `RouteMeta` has at least one integration test") is practical. BRRTRouter's generated handler model makes this particularly feasible.

---

## The Radix Trie Proposal

The second half of the document provides a **complete, production-ready radix trie implementation** for O(#segments) route matching with zero allocations.

**Opinion:** ✅ **EXCELLENT - Worth implementing**

Key strengths:
1. **Cache-friendly design** - Nodes in contiguous arena, segments in packed buffer
2. **Bounded inline children** - No unbounded allocations
3. **Priority ordering** - static → param → wildcard (matches httprouter semantics)
4. **Real Rust code** - Not pseudocode, directly usable

**Trade-off:** This would be a significant refactor of BRRTRouter's current matcher. The current implementation is simpler but scales as O(Routes × Segments).

**Recommendation:** 
- Phase 1: Implement the JSF-SAFE lint profile immediately (low effort, high value)
- Phase 2: Benchmark current router at scale (1000+ routes) to quantify need
- Phase 3: If latency is a concern, implement radix trie

---

## Missing Considerations

The document could be strengthened by addressing:

1. **May coroutine stack sizing** - How does "no recursion" interact with May's per-coroutine stack limits? (BRRTRouter already addresses this with per-handler stack sizing)

2. **Hot reload interaction** - The trie is "immutable after build" but BRRTRouter supports hot reload. Need to address atomicity of trie swaps.

3. **Metrics overhead** - Current BRRTRouter has metrics middleware. Are metrics collection patterns JSF-compliant?

---

## Actionable Next Steps

### Immediate (This Sprint)
1. Add `#![deny(...)]` profile to `src/router/core.rs` and `src/dispatcher/core.rs`
2. Create `docs/BRRTRouter-SAFE.md` formalizing the rules
3. Add CI gate for CC threshold

### Short-term (Next 2 Sprints)
1. Audit hot path for allocations; refactor obvious violations
2. Add integration test that hits every generated route
3. Benchmark route matching latency

### Long-term (If Needed)
1. Implement radix trie as optional feature flag
2. Compare performance metrics pre/post trie

---

## Conclusion

This document represents **serious, thoughtful engineering** that applies hard-won safety-critical systems knowledge to modern Rust. The author clearly understands both the JSF rules' intent and Rust's idioms.

The "BRRTRouter-SAFE" profile should become a formal part of the project's development standards. It would position BRRTRouter as one of the few Rust routers with documented, enforced determinism guarantees - valuable for financial services, embedded systems, and other latency-sensitive domains.

**Verdict: Implement the linting profile immediately. Defer radix trie until benchmarks prove necessity.**

---

*This audit was conducted by reviewing the provided documentation against BRRTRouter's current architecture and industry best practices for high-performance systems.*

