# URI / Request-Target — Build Board

**Theme:** Epics 10–11  
**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md) (positive + negative unit tests mandatory)  
**Wiki:** [`llmwiki/topics/uri-request-target-compliance.md`](../../../llmwiki/topics/uri-request-target-compliance.md)

Track implementation here. Update **Status** as work lands (`todo` → `doing` → `done`).

## Now / next

| Priority | ID | Status | Issue | Notes |
|----------|-----|--------|-------|-------|
| **NOW** | 10.1 | todo | [#375](https://github.com/microscaler/BRRTRouter/issues/375) | Spec matrix + golden corpus — unblocks Wave 1 |
| NEXT | 10.2 | todo | [#376](https://github.com/microscaler/BRRTRouter/issues/376) | Inbound query (parallel with 10.3, 10.11) |
| NEXT | 10.3 | todo | [#377](https://github.com/microscaler/BRRTRouter/issues/377) | Inbound path decode |
| NEXT | 10.11 | todo | [#385](https://github.com/microscaler/BRRTRouter/issues/385) | may_minihttp boundary contract |

## Wave plan (Epic 10)

```text
Wave 0 ──► 10.1 matrix/goldens
              │
Wave 1 ──► 10.2 query ‖ 10.3 path ‖ 10.11 boundary
              │
Wave 2 ──► 10.4 component encoders
              │
Wave 3 ──► 10.5 passthrough ‖ 10.6 414 limits
              │
Wave 4 ──► 10.7 error taxonomy (stop 502 for composition)
              │
Wave 5 ──► 10.9 OpenAPI style → 10.10 fuzz → 10.8 unify http stack
```

| Wave | Stories | Depends on | Outcome |
|------|---------|------------|---------|
| 0 | 10.1 | — | Executable Done definition |
| 1 | 10.2, 10.3, 10.11 | 10.1 | Inbound parse + boundary contract |
| 2 | 10.4 | 10.1, 10.2 | Named encoders; provinces-class locked |
| 3 | 10.5, 10.6 | 10.2, 10.4, 10.11 (for 10.5) | Passthrough + 414 |
| 4 | 10.7 | 10.6 | Composition ≠ 502 |
| 5 | 10.9 → 10.10 → 10.8 | 10.2–10.5, 10.9 | Fidelity + proof + stack unify |

## Epic 11 (parallel after 10.1; 11.3 after 10.7)

| Order | ID | Status | Issue | Blocked by |
|-------|-----|--------|-------|------------|
| 1 | 11.1 | todo | [#386](https://github.com/microscaler/BRRTRouter/issues/386) | 10.1 (soft) |
| 2 | 11.2 | todo | [#387](https://github.com/microscaler/BRRTRouter/issues/387) | 11.1 |
| 3 | 11.3 | todo | [#388](https://github.com/microscaler/BRRTRouter/issues/388) | 11.1, **10.7** |
| 4 | 11.4 | todo | [#389](https://github.com/microscaler/BRRTRouter/issues/389) | 11.2 |

## Full story index

| ID | Title | Wave | Status | GitHub |
|----|--------|------|--------|--------|
| Epic 10 | Request-target parse & rebuild | — | todo | [#373](https://github.com/microscaler/BRRTRouter/issues/373) |
| 10.1 | Spec matrix & golden corpus | 0 | todo | [#375](https://github.com/microscaler/BRRTRouter/issues/375) |
| 10.2 | Inbound query parse edge cases | 1 | todo | [#376](https://github.com/microscaler/BRRTRouter/issues/376) |
| 10.3 | Inbound path segment decode | 1 | todo | [#377](https://github.com/microscaler/BRRTRouter/issues/377) |
| 10.4 | Component-specific encoders | 2 | todo | [#378](https://github.com/microscaler/BRRTRouter/issues/378) |
| 10.5 | Proxy path/query passthrough | 3 | todo | [#379](https://github.com/microscaler/BRRTRouter/issues/379) |
| 10.6 | Request-target length → 414 | 3 | todo | [#380](https://github.com/microscaler/BRRTRouter/issues/380) |
| 10.7 | Error taxonomy | 4 | todo | [#381](https://github.com/microscaler/BRRTRouter/issues/381) |
| 10.8 | Unify http URI stack | 5 | todo | [#382](https://github.com/microscaler/BRRTRouter/issues/382) |
| 10.9 | OpenAPI style/explode fidelity | 5 | todo | [#383](https://github.com/microscaler/BRRTRouter/issues/383) |
| 10.10 | Property/fuzz compliance suite | 5 | todo | [#384](https://github.com/microscaler/BRRTRouter/issues/384) |
| 10.11 | may_minihttp request-line boundary | 1 | todo | [#385](https://github.com/microscaler/BRRTRouter/issues/385) |
| Epic 11 | HTTP QUERY (RFC 10008) | — | todo | [#374](https://github.com/microscaler/BRRTRouter/issues/374) |
| 11.1 | Method + router + CORS | E11 | todo | [#386](https://github.com/microscaler/BRRTRouter/issues/386) |
| 11.2 | OpenAPI QUERY operations | E11 | todo | [#387](https://github.com/microscaler/BRRTRouter/issues/387) |
| 11.3 | Proxy & client QUERY | E11 | todo | [#388](https://github.com/microscaler/BRRTRouter/issues/388) |
| 11.4 | Accept-Query + POST fallback docs | E11 | todo | [#389](https://github.com/microscaler/BRRTRouter/issues/389) |

## Definition of Done (per story)

1. Delivery items in the story doc landed.
2. **Unit tests** tables (positive + negative) implemented under `cargo test`.
3. Story + BUILD_BOARD status → `done`; GitHub issue closed.
4. Epic 10 Done only when Wave 5 complete and audit scorecard has no Gap rows for parse/rebuild.
