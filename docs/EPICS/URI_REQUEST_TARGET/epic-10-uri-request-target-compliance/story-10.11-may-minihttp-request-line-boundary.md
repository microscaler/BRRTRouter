# Story 10.11 — may_minihttp request-line boundary

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1  
**Blocks:** 10.3, 10.5

## Overview

Compliance depends on knowing the **exact** path/query string may_minihttp
passes into `parse_request`. Fragment stripping, absolute-form vs origin-form,
and normalization must be documented and tested at the boundary — otherwise
BRRTRouter unit tests can pass while production bytes differ.

## Delivery

- Document request-line → `raw_path` contract in may_minihttp and BRRTRouter
  (`docs/` + code comments at `parse_request`).
- Integration or contract tests: origin-form with query; assert `parse_query_params`
  input matches expected octets.
- Note absolute-URI form / OPTIONS `*` if supported.
- If may_minihttp must change to expose raw query bytes for 10.5, open a linked
  issue on that repo and track it here.

## Acceptance criteria

- [ ] Written contract: what characters can appear in `raw_path` (query? fragment?).
- [ ] At least one integration test spanning may_minihttp → `parse_query_params`.
- [ ] Gaps that require may_minihttp changes are filed and linked.
- [ ] Matrix row for request-line boundary marked Pass.

## References

- may_minihttp server request parsing
- `src/server/request.rs` `parse_request`
- RFC 9110 §7.1 request target forms
