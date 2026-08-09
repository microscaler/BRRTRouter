# Story 13.8 — Response compression middleware

**GitHub issue:** [#408](https://github.com/microscaler/BRRTRouter/issues/408)  
**Epic:** [Epic 13](README.md)  
**Wave:** 4  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Replace aspirational `CompressionMiddleware` docs with a real **opt-in** gzip
(and optionally brotli) response compression middleware for eligible content types,
or permanently document “not supported” if deferred — but this story’s target is
to **ship gzip**.

## Delivery

- Middleware negotiates `Accept-Encoding`.
- Compress JSON/`text/*` responses above a minimum size threshold.
- Skip already-compressed types (`image/*`, `gzip`, SSE streams).
- Opt-in via `AppConfig` (default **off** for CPU predictability).
- Metrics: compressed bytes / responses optional.
- Update RequestLifecycle only after ship.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | With gzip enabled and `Accept-Encoding: gzip`, large JSON body is gzip-encoded. |
| FR-2 | Response includes `Content-Encoding: gzip` when compressed. |
| FR-3 | Client without gzip support receives identity encoding. |
| FR-4 | SSE / `text/event-stream` is never gzip-compressed. |
| FR-5 | Disabled config → identity always. |
| FR-6 | Small responses below threshold skip compression. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | Default off — no surprise CPU on hot path. |
| NFR-2 | No panic on empty body. |
| NFR-3 | Compression failures fall back to identity or 500 (documented); no truncate. |
| NFR-4 | Does not break Content-Length semantics (adjust or switch to chunked — document). |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Large JSON + Accept-Encoding gzip | Content-Encoding gzip; body decodes |
| P2 | No Accept-Encoding | identity |
| P3 | Disabled config | identity |
| P4 | Below size threshold | identity |
| P5 | `text/plain` eligible | compressible |
| P6 | Round-trip gunzip equals original | equal |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | SSE content-type | never compressed |
| N2 | `image/png` | never compressed |
| N3 | Panic on compress | forbidden |
| N4 | Partial/corrupt body served as success | forbidden |
| N5 | Compress when client rejects gzip | forbidden |
| N6 | Silent corruption of Unicode JSON | forbidden |

### Acceptance criteria (tests)

- [x] P1/P2 and N1/N3 mandatory.

## Acceptance criteria

- [x] Middleware ships opt-in; docs accurate.
- [x] FR/NFR + unit tests complete.

## References

- `docs/RequestLifecycle.md` middleware table
- `src/middleware/mod.rs`
