# BRRTRouter OTEL Logging Audit + PRD

## Scope
Audit current BRRTRouter tracing/logging implementation and define a product requirements document to improve debug usefulness while protecting sensitive data (especially password-like fields).

---

## Executive Summary

Current logging is strong on request lifecycle events but weak on **safe payload visibility** and **noise control**:

- Sensitive data redaction is configured but not actually enforced at event field-value level.
- Debug logs include full header/cookie/query value dumps, which is high-risk and hard to use at scale.
- Request context is fragmented across modules/spans, and key correlation fields are inconsistently attached.
- OTEL export is documented as “future” and not actively wired in runtime init.
- Third-party runtime logs (e.g., `may::*`) can still leak into output depending on env filter shape.

---

## Audit: Touchpoints and Findings

### 1) Logging bootstrap / subscriber stack

**Touchpoints**
- `/home/runner/work/BRRTRouter/BRRTRouter/src/otel.rs` (config, layers, formatter, env filters)
- `/home/runner/work/BRRTRouter/BRRTRouter/examples/pet_store/src/main.rs` (init call)
- `/home/runner/work/BRRTRouter/BRRTRouter/templates/main.rs.txt`
- `/home/runner/work/BRRTRouter/BRRTRouter/templates/impl_main.rs.txt`

**Findings**
- Redaction layer exists but `on_event` is effectively a no-op (placeholder comments).
- `rate_limit_rps` and `buffer_size` are parsed but not used as real control knobs.
- JSON formatting enables both `current_span` and `span_list`, producing verbose/duplicated nested output.
- `OTEL_EXPORTER_OTLP_ENDPOINT` is set in deployments, but runtime code does not currently wire an OTLP exporter/provider.

### 2) Request parsing logs (input-level observability)

**Touchpoints**
- `/home/runner/work/BRRTRouter/BRRTRouter/src/server/request.rs`

**Findings**
- Good: header/query/cookie/body parsing emits structured events.
- Gap: logs expose names/counts but not controlled value previews for debugging.
- Risk: body parsing log can still indirectly expose sensitive shape without policy.
- No centralized sanitizer for JSON body fields (e.g., password/token/secret fields).

### 3) Request service logs (ingress/egress path)

**Touchpoints**
- `/home/runner/work/BRRTRouter/BRRTRouter/src/server/service.rs`

**Findings**
- `Request received` debug log currently dumps full headers/query/cookies; dangerous for auth/session/token leakage.
- Request ID is generated/propagated, but not consistently embedded in all request lifecycle logs/spans.
- Span defines `status` as empty but never records it.
- Route miss (`404`) returns response but has no explicit “route_not_found” log event.

### 4) Dispatcher / middleware logs

**Touchpoints**
- `/home/runner/work/BRRTRouter/BRRTRouter/src/dispatcher/core.rs`
- `/home/runner/work/BRRTRouter/BRRTRouter/src/middleware/tracing.rs`
- `/home/runner/work/BRRTRouter/BRRTRouter/src/worker_pool.rs`

**Findings**
- Dispatcher emits useful operational logs, but request path context can switch to route pattern form and fragment debugging.
- Tracing middleware creates extra spans/events and can duplicate lifecycle verbosity relative to service-level spans.
- Worker pool panic logs are present but lack standardized correlation envelope.

### 5) Deployment/runtime log controls

**Touchpoints**
- `/home/runner/work/BRRTRouter/BRRTRouter/k8s/app/base/deployment.yaml`
- `/home/runner/work/BRRTRouter/BRRTRouter/k8s/app/deployment.yaml`
- `/home/runner/work/BRRTRouter/BRRTRouter/justfile`

**Findings**
- `RUST_LOG` includes `may=warn` in manifests, but in practice environment overrides can still admit noisy `may::*` info logs.
- No explicit first-class env controls for request payload logging mode (off/meta/safe_preview/full_dev_only).

### 6) Test coverage for observability behavior

**Touchpoints**
- `/home/runner/work/BRRTRouter/BRRTRouter/tests/tracing_tests.rs`
- `/home/runner/work/BRRTRouter/BRRTRouter/tests/tracing_util.rs`

**Findings**
- Existing tracing tests verify span emission only.
- Missing tests for redaction policy, sensitive-field masking/fuzzing, and log-schema completeness.

---

## Problem Statement (Refined)

BRRTRouter needs logs that are simultaneously:
1. **Actionable for debugging** (path, route, request context, failure reason, bounded payload insight),
2. **Safe by default** (password/token/secret data never leaked),
3. **Low-noise in production** (runtime/system logs filtered, correlated, queryable).

---

## PRD: OTEL Logging Improvements

## Goals
- Provide complete request debugging context with deterministic structured fields.
- Enforce automatic sensitive data masking/fuzzing for request payloads and credential-bearing fields.
- Reduce noisy system-level logs and duplicated span payload.
- Keep hot-path overhead bounded and configurable.

## Non-Goals
- Full distributed tracing backend rollout across all environments.
- Replacing current logger framework or moving away from `tracing`.

## Functional Requirements

### FR1: Sensitive field sanitizer (required)
- Introduce a centralized sanitizer used by service/request/dispatcher logging.
- Field-name based policy (case-insensitive) for: `password`, `passwd`, `pwd`, `secret`, `token`, `api_key`, `authorization`, etc.
- JSON body traversal must mask nested sensitive keys.
- Output mode for sensitive values: deterministic fuzzing/masking (`abcd***` or `<REDACTED>`), never raw.

### FR2: Request payload logging modes (required)
- Add config: payload logging mode with options:
  - `off` (default prod),
  - `meta` (counts/types only),
  - `safe_preview` (sanitized + truncated),
  - `full` (dev-only, explicit opt-in).
- Apply to body/query/cookies/headers independently where needed.

### FR3: Correlation envelope normalization (required)
- Every request lifecycle event must include:
  - `request_id`,
  - `method`,
  - `path` (actual),
  - `route_pattern` (when matched),
  - `status` (on completion),
  - `duration_ms`.
- Ensure span `status` field is recorded before completion.

### FR4: Noise suppression policy (required)
- Add explicit default directive for noisy runtime targets (including `may::io::sys::select`) unless overridden.
- Document precedence between `RUST_LOG`, BRRTR filters, and per-target overrides.

### FR5: JSON schema consistency (required)
- Define canonical JSON log field names and event names for:
  - request_start,
  - auth_result,
  - validation_failure,
  - route_not_found,
  - request_complete.
- Remove duplicate span dumps by making span list verbosity configurable.

### FR6: Validation and tests (required)
- Add tests for:
  - sanitizer behavior across nested JSON,
  - password/token masking in headers/query/body logs,
  - required correlation fields in key events,
  - noise filter directives behavior.

## Non-Functional Requirements
- No raw sensitive values at default settings.
- Log emission changes must not materially regress hot-path latency.
- Backward-compatible defaults for existing deployments (except safety hardening).

---

## Implementation Touchpoint Plan (by file)

1. **Core policy + config**
   - `src/otel.rs` (real redaction hook, mode flags, filter defaults, verbosity toggles)
2. **Request parsing events**
   - `src/server/request.rs` (sanitize/truncate previews; structured payload metadata)
3. **Request lifecycle and completion**
   - `src/server/service.rs` (normalized envelope, status recording, route-not-found event)
4. **Dispatch consistency**
   - `src/dispatcher/core.rs`, `src/worker_pool.rs` (request_id + path/route correlation alignment)
5. **Runtime templates/deploy**
   - `templates/main.rs.txt`, `templates/impl_main.rs.txt`, `k8s/app/*deployment*.yaml`
6. **Tests**
   - `tests/tracing_tests.rs` + new targeted sanitizer/log-policy tests

---

## Rollout Strategy

1. **Phase 1 (Safety first)**: implement sanitizer + enforce masking in existing events.
2. **Phase 2 (Signal quality)**: normalize envelope, status recording, route-not-found event.
3. **Phase 3 (Operational tuning)**: noise filters + configurable span verbosity + payload modes.
4. **Phase 4 (Hardening)**: exhaustive tests + docs updates + migration notes.

---

## Acceptance Criteria

- No event contains raw values for keys matching sensitive policy under default config.
- A request with JSON body containing `password` logs masked/fuzzed value only.
- Request completion logs include request_id, method, path, route_pattern (if matched), status, duration.
- `may::io::sys::select` info-level noise is suppressed by default in production profile.
- Test suite includes automated checks for sanitizer and correlation envelope requirements.

---

## Risks / Open Questions

- Exact masking strategy (strict `<REDACTED>` vs partial fuzzing) should be finalized with security stakeholders.
- Payload preview size limits should balance debugging utility vs memory/latency overhead.
- If full OTLP export is required now (not future), a separate OTel exporter integration story is needed.
