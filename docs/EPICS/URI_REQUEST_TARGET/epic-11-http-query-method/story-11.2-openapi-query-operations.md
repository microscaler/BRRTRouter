# Story 11.2 — OpenAPI QUERY operations

**GitHub issue:** _(create)_  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Blocked by:** 11.1

## Overview

Spec load + generator recognize QUERY operations with request bodies (query media types).

## Delivery

- Spec parser: allow `QUERY` in path item operations (as extension or when OpenAPI tooling supports it).
- Generator/Askama: emit handlers for QUERY + body schemas.
- Document how consumers declare QUERY in suite configs until OAS formally lists it everywhere.

## Acceptance criteria

- [ ] Example OpenAPI snippet in docs loads without error.
- [ ] Generated handler receives body for QUERY.
- [ ] Unknown/unsupported tooling path documented.

## References

- RFC 10008 §2.1 (media types)
- `src/spec/`, generator templates
