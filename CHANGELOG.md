# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Observability: Introduced typed `RequestId` (ULID-backed) with end-to-end correlation.
  - Server now accepts inbound `X-Request-ID` (validated) or generates a ULID.
  - Server always echoes `X-Request-ID` on responses.
  - `HandlerRequest.request_id` migrated to a typed `RequestId` newtype.
  - Dispatcher gained `dispatch_with_request_id(...)` to pass correlation IDs explicitly.
  - `RequestLogger` now captures `request_id` at start; completion logs include it reliably.
- Tests updated to construct `RequestId` without parsing dummy strings.
- Docs: Updated `docs/LogAnalysis.md` and `docs/wip/LOGGING_PRD.md` for ULID `request_id` and header propagation; clarified that `request_id` must not be used as a Prometheus label.

### Changed
- Metrics test aligned to labeled series format for `brrtrouter_requests_total`.

### Notes
- This change is backward compatible for log consumers: the `request_id` field remains, now populated with a ULID string.
- Prometheus dashboards are unaffected (no new labels introduced).
