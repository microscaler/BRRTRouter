# Story 13.4 — Streaming uploads & download helpers

**GitHub issue:** [#404](https://github.com/microscaler/BRRTRouter/issues/404)  
**Epic:** [Epic 13](README.md)  
**Wave:** 2  
**Effort:** L  
**Blocked by:** benefits from 12.2 body caps + 12.6 multipart MVP-A  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Move file uploads beyond buffered multipart MVP-A: **stream file parts to disk
(or a sink)** under size caps, and provide **download response helpers**
(`Content-Disposition`, binary content-type) for OpenAPI `format: binary` /
file responses.

## Delivery

- Streaming multipart file part reader with max part size (413 on exceed).
- Temp file / configurable directory; cleanup on success/failure paths.
- Handler-visible handle (path or reader) — document ownership.
- Download helpers: `HttpFile` / `attachment()` setting disposition + content-type.
- OpenAPI `encoding` object: document supported subset (or explicit ignore list).
- Keep text fields → JSON behavior from 12.6.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | File part larger than buffered MVP threshold can stream to a sink without holding full bytes in a `String`. |
| FR-2 | Exceeding part/global cap → **413**; no partial commit to app state. |
| FR-3 | Text fields still available as JSON object keys to the handler. |
| FR-4 | Download helper sets `Content-Disposition` (attachment/inline) and `Content-Type`. |
| FR-5 | Missing boundary / malformed multipart still **400** (12.6 regression). |
| FR-6 | Binary response schemas are not falsely JSON-schema-validated as UTF-8 JSON. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | No panic on truncated multipart / hostile boundaries. |
| NFR-2 | Temp files removed on error paths (or documented leak budget + scrub). |
| NFR-3 | Streaming path honors 12.2 global/route body limits. |
| NFR-4 | Default temp dir is secure (permissions / `TMPDIR`). |
| NFR-5 | Do not log raw file bytes. |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Small file part streams to temp; handler sees path/size | ok |
| P2 | Text + file mixed multipart | both available |
| P3 | Download helper disposition=attachment | header set |
| P4 | Under size cap | 2xx path |
| P5 | UTF-8 text field unchanged from 12.6 | regression |
| P6 | Content-Type on download helper | matches arg |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | File over part cap | **413** |
| N2 | Malformed multipart | **400**; no panic |
| N3 | Missing boundary | **400** |
| N4 | Disk full / sink error | Err/5xx; no panic; cleanup attempted |
| N5 | Silent truncate of file part | forbidden |
| N6 | Path traversal in temp name | forbidden |
| N7 | Panic on empty file part | forbidden |

### Acceptance criteria (tests)

- [ ] P1/P2 and N1/N2 mandatory.

## Acceptance criteria

- [ ] Streaming upload API documented (`docs/multipart.md` updated).
- [ ] Download helper usable from typed handlers.
- [ ] FR/NFR + unit tests complete.

## References

- `src/server/multipart.rs`, `docs/multipart.md`, `docs/request_body_limits.md`
