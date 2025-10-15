# Logging System - Product Requirements Document (PRD)

**Project**: BRRTRouter  
**Document Version**: 1.0  
**Date**: October 2025  
**Status**: Draft for Review  
**Author**: System Architecture Team

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Logging Touchpoints](#2-logging-touchpoints)
3. [JSON Log Format Specification](#3-json-log-format-specification)
4. [Sensitive Data Handling](#4-sensitive-data-handling)
5. [Configuration Specification](#5-configuration-specification)
6. [Implementation Guidance](#6-implementation-guidance)
7. [Success Metrics](#7-success-metrics)
8. [Open Questions](#8-open-questions)
9. [Appendices](#9-appendices)

---

## 1. Executive Summary

### 1.1 Problem Statement

BRRTRouter currently has **inconsistent and incomplete logging** across its codebase:

- **82 logging calls** across 22 files
- Mix of `println!`, `eprintln!`, and `tracing` macros
- No standardized JSON format for production observability
- Missing logs at critical touchpoints (security failures, handler panics, validation errors)
- **Sensitive data** (API keys, tokens) logged without redaction
- No structured correlation between logs and distributed traces
- Difficult to debug production issues without comprehensive context

**Impact**: 
- Slow incident response times (no context in logs)
- Security compliance risks (PII/credentials in logs)
- Poor integration with Loki/Prometheus observability stack
- Inconsistent developer experience across modules

### 1.2 Goals

Implement comprehensive, structured logging using the existing **`tracing` crate** to:

1. **Observability**: 100% coverage of critical touchpoints with rich context
2. **Security**: Automatic redaction of credentials and PII
3. **Performance**: <5% latency overhead using async buffering
4. **Flexibility**: Configurable sampling, rate limiting, and log levels
5. **Integration**: Seamless Loki/Jaeger/Prometheus integration

### 1.3 Key Requirements

| Requirement | Specification |
|-------------|---------------|
| **Logging Framework** | Use existing `tracing` crate (v0.1.40+) with `tracing-subscriber` |
| **Log Format** | Structured JSON (production) with optional pretty-print (development) |
| **Sensitive Data** | Mask credentials by default; configurable redaction levels |
| **Sampling** | Configurable: log all, sample successes, or sample by rate limit |
| **Performance** | Async buffering with configurable buffer size (default: 8192) |
| **Correlation** | Trace ID, span ID, parent span ID in every log |
| **Cost** | <1000 logs/sec at 10% sampling on 40k req/s throughput |

### 1.4 Success Criteria

- ✅ All 50+ critical touchpoints have structured logs
- ✅ Zero credentials/PII leaked in production logs
- ✅ <5% performance degradation with async logging
- ✅ Debug time reduced by 50% (measured via incident resolution)
- ✅ Loki integration working with proper labels
- ✅ 100% test coverage for logging utilities

### 1.5 Non-Goals

- ❌ Not replacing OpenTelemetry tracing (complementary)
- ❌ Not implementing custom log shipping (use Loki/Promtail)
- ❌ Not adding log analytics UI (use Grafana/Loki)
- ❌ Not implementing real-time log streaming endpoints

---

## 2. Logging Touchpoints

This section documents all logging touchpoints across BRRTRouter, categorized by system component.

### 2.1 Summary Statistics

| Component | Touchpoints | Critical | High | Medium | Low |
|-----------|-------------|----------|------|--------|-----|
| Request Lifecycle | 8 | 2 | 3 | 2 | 1 |
| Routing | 6 | 1 | 2 | 2 | 1 |
| Security | 12 | 5 | 4 | 2 | 1 |
| Validation | 8 | 3 | 3 | 2 | 0 |
| Dispatcher | 7 | 3 | 2 | 1 | 1 |
| Handler Execution | 6 | 3 | 2 | 1 | 0 |
| Response Flow | 5 | 1 | 2 | 2 | 0 |
| Middleware | 6 | 0 | 2 | 3 | 1 |
| Startup | 9 | 2 | 4 | 2 | 1 |
| Hot Reload | 4 | 1 | 2 | 1 | 0 |
| Code Generation | 7 | 1 | 3 | 2 | 1 |
| **TOTAL** | **78** | **22** | **29** | **20** | **7** |


### 2.1.1 Logging Destination Strategy

**IMPORTANT**: Not all touchpoints are logged to the same destination. Logging strategy differs based on component lifecycle:

| Component Category | Destination | Rationale |
|--------------------|-------------|-----------|  
| **Runtime Components** | `tracing-subscriber` → Loki | Observability for production incidents |
| **Development Workflows** | Local stdout/stderr only | Build-time operations, not runtime issues |

**Runtime Components (Phases 1-5):**
- ✅ Request Lifecycle (R1-R8)
- ✅ Routing (RT1-RT6)
- ✅ Security (S1-S12)
- ✅ Validation (V1-V8)
- ✅ Dispatcher (D1-D7)
- ✅ Handler Execution (H1-H6)
- ✅ Response Flow (RF1-RF5)
- ✅ Middleware (M1-M6)
- ✅ **Hot Reload (HR1-HR4)** ← Goes to BOTH stdout AND tracing-subscriber

**Development Workflows (Documented but Local-Only):**
- 📝 **Startup (ST1-ST9)** ← Local stdout only (bootstrap logging)
- 📝 **Code Generation (CG1-CG7)** ← Local stdout only (dev-time tool output)

**Rationale:**
- **Hot Reload**: Runtime spec changes affect live traffic → needs observability
- **Startup**: Happens once before serving → simple stdout sufficient  
- **Code Generation**: Build-time tool → not a production concern

**Implementation Note**: Phases 1-5 implement runtime logging with `tracing` macros. Phase 5 implements HR1-HR4 only. ST/CG touchpoints remain documented for reference but use simple `println!` for developer feedback.
### 2.2 Request Lifecycle Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| R1 | TCP connection accepted | Low | DEBUG | Per-connection | `remote_addr`, `local_addr` | Missing | `server/service.rs:handle_request` |
| R2 | HTTP request parsed | High | INFO | Per-request | `method`, `path`, `http_version`, `headers_count` | Incomplete | `server/service.rs:532` |
| R3 | Headers extracted | Medium | DEBUG | Per-request | `header_names[]`, `header_count`, `size_bytes` | Missing | `server/service.rs` |
| R4 | Query params parsed | Medium | DEBUG | Per-request | `query_params{}`, `param_count` | Missing | `server/service.rs` |
| R5 | Request body read | High | INFO | Per-POST/PUT | `content_length`, `content_type`, `body_size_bytes` | Missing | `server/service.rs` |
| R6 | JSON body parsed | High | DEBUG | Per-POST/PUT | `body_json` (redacted), `parse_duration_ms` | Missing | `server/service.rs` |
| R7 | Cookies extracted | Medium | DEBUG | Per-request | `cookie_count`, `cookie_names[]` | Missing | `server/service.rs` |
| R8 | Request complete | Critical | INFO | Per-request | `request_id`, `total_size_bytes`, `parse_duration_ms` | Partial | `server/service.rs:491` |

**Notes**:
- R2 currently has debug logging but missing critical fields
- R8 exists but lacks structured context (request ID, sizes)
- All missing touchpoints need to be added during implementation

### 2.3 Routing Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| RT1 | Route match attempt | Medium | DEBUG | Per-request | `method`, `path`, `routes_count` | Missing | `router/core.rs:route()` |
| RT2 | Regex match success | Low | DEBUG | Per-request | `pattern`, `captures{}`, `params_extracted` | Missing | `router/core.rs:155` |
| RT3 | Route matched | High | INFO | Per-request | `handler_name`, `path_params{}`, `route_pattern` | Missing | `router/core.rs:162` |
| RT4 | No route found (404) | Critical | WARN | Per-404 | `method`, `path`, `attempted_patterns[]` | Missing | `server/service.rs` |
| RT5 | Routing table loaded | High | INFO | Per-startup | `routes_count`, `base_path`, `routes_summary[]` | Partial | `router/core.rs:109` |
| RT6 | Route sorting applied | Medium | DEBUG | Per-startup | `routes_before`, `routes_after`, `sort_strategy` | Missing | `router/core.rs:86` |

**Notes**:
- RT5 exists (`dump_routes`) but uses `println!` instead of `tracing`
- RT4 is critical for debugging 404 issues but currently has no logging

### 2.4 Security Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| S1 | Security check start | High | DEBUG | Per-protected-request | `handler`, `schemes_required[]`, `scopes_required[]` | Missing | `server/service.rs:608` |
| S2 | Security scheme lookup | Medium | DEBUG | Per-scheme | `scheme_name`, `scheme_type` | Missing | `server/service.rs:619` |
| S3 | Provider not found | High | ERROR | On-error | `scheme_name`, `available_providers[]` | Missing | `server/service.rs:627` |
| S4 | Provider validation start | Medium | DEBUG | Per-scheme | `provider_type`, `scopes[]` | Missing | `security.rs` |
| S5 | Token/key extracted | Medium | DEBUG | Per-validation | `source` (header/cookie/query), `key_prefix` (first 4 chars) | Missing | `security.rs` |
| S6 | Validation success | High | INFO | Per-success | `scheme_name`, `user_id` (if available), `scopes_granted[]` | Partial | `server/service.rs:707` |
| S7 | Validation failed | Critical | WARN | Per-failure | `scheme_name`, `reason`, `attempted_schemes[]` | Partial | `server/service.rs:700,702` |
| S8 | Insufficient scope (403) | Critical | WARN | Per-403 | `scheme_name`, `scopes_required[]`, `scopes_granted[]` | Partial | `server/service.rs:702` |
| S9 | JWKS fetch success | Medium | INFO | Per-fetch | `jwks_url`, `keys_count`, `cache_ttl_secs` | Missing | `security.rs:JwksBearerProvider` |
| S10 | JWKS fetch failure | High | ERROR | On-error | `jwks_url`, `error`, `retry_attempt`, `will_retry` | Missing | `security.rs:445` |
| S11 | Remote API key verified | Medium | DEBUG | Per-verification | `verify_url`, `cached`, `cache_ttl_secs` | Missing | `security.rs:RemoteApiKeyProvider` |
| S12 | Security metrics recorded | Low | DEBUG | Per-request | `auth_failures_total`, `auth_latency_ms` | Missing | `server/service.rs:662` |

**Notes**:
- S6, S7, S8 are partially implemented but lack structured fields
- S10 is critical for debugging auth issues but currently has no logging
- All provider-specific validation needs instrumentation

### 2.5 Validation Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| V1 | Request validation start | High | DEBUG | Per-request | `handler`, `schema_present`, `required_fields[]` | Missing | `server/service.rs:714` |
| V2 | Required body missing | High | WARN | On-error | `handler`, `expected_content_type` | Missing | `server/service.rs:710` |
| V3 | Schema validation failed | Critical | WARN | On-error | `handler`, `errors[]`, `schema_path`, `invalid_fields[]` | Missing | `server/service.rs:717` |
| V4 | Path param validation | Medium | DEBUG | Per-request | `param_name`, `param_value`, `param_type`, `valid` | Missing | `router/core.rs` |
| V5 | Query param validation | Medium | DEBUG | Per-request | `param_name`, `param_value`, `constraints{}`, `valid` | Missing | `server/service.rs` |
| V6 | Response validation start | Medium | DEBUG | Per-response | `handler`, `status`, `schema_present` | Missing | `typed/core.rs` |
| V7 | Response validation failed | High | ERROR | On-error | `handler`, `status`, `errors[]`, `schema_path` | Missing | `typed/core.rs` |
| V8 | Type conversion failed | High | WARN | On-error | `handler`, `expected_type`, `actual_value`, `error` | Missing | `typed/core.rs:84` |

**Notes**:
- V3 is critical but currently only returns error to client (no logging)
- V7 and V8 are important for catching bugs in handler implementation
- Currently validation failures are silent in logs

### 2.6 Dispatcher Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| D1 | Handler lookup | Medium | DEBUG | Per-request | `handler_name`, `found` | Missing | `dispatcher/core.rs:185` |
| D2 | Handler not found | Critical | ERROR | On-error | `handler_name`, `available_handlers[]` | Missing | `dispatcher/core.rs:185` |
| D3 | Request dispatched | High | DEBUG | Per-request | `handler_name`, `request_size_bytes`, `channel_depth` | Missing | `dispatcher/core.rs:210` |
| D4 | Middleware before | Low | DEBUG | Per-middleware | `middleware_name`, `short_circuit` | Missing | `dispatcher/core.rs:199` |
| D5 | Middleware after | Low | DEBUG | Per-middleware | `middleware_name`, `latency_ms` | Missing | `dispatcher/core.rs:215` |
| D6 | Response received | High | DEBUG | Per-request | `handler_name`, `status`, `latency_ms`, `response_size_bytes` | Missing | `dispatcher/core.rs:211` |
| D7 | Handler timeout | Critical | ERROR | On-timeout | `handler_name`, `timeout_ms`, `request` (redacted) | Missing | `dispatcher/core.rs` |

**Notes**:
- D2 and D7 are critical error paths with no logging
- D3 and D6 are essential for request tracing
- Channel depth could indicate backpressure issues

### 2.7 Handler Execution Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| H1 | Handler coroutine start | Medium | DEBUG | Per-request | `handler_name`, `coroutine_id` | Missing | `dispatcher/core.rs:126` |
| H2 | Handler execution start | High | INFO | Per-request | `handler_name`, `path_params{}`, `query_params{}` | Missing | Handler functions |
| H3 | Handler panic caught | Critical | ERROR | On-panic | `handler_name`, `panic_message`, `backtrace` | Partial | `dispatcher/core.rs:146` |
| H4 | Handler execution complete | High | INFO | Per-request | `handler_name`, `status`, `execution_time_ms` | Missing | Handler functions |
| H5 | Database query executed | Medium | DEBUG | Per-query | `query_type`, `table`, `duration_ms`, `rows_affected` | Missing | Handler functions |
| H6 | External API called | High | INFO | Per-call | `api_url`, `method`, `status`, `duration_ms`, `retry_count` | Missing | Handler functions |

**Notes**:
- H3 uses `eprintln!` instead of structured logging
- H2 and H4 are user-implemented but guidance needed in generated handlers
- H5 and H6 are handler-specific and need documentation/examples

### 2.8 Response Flow Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| RF1 | Response serialization start | Medium | DEBUG | Per-request | `status`, `body_size_bytes`, `format` (json/html) | Missing | `server/service.rs` |
| RF2 | JSON serialization failed | High | ERROR | On-error | `error`, `body_type`, `attempted_value` (redacted) | Missing | `server/service.rs` |
| RF3 | Response headers built | Low | DEBUG | Per-request | `headers_count`, `total_size_bytes` | Missing | `server/service.rs` |
| RF4 | HTTP response written | Medium | DEBUG | Per-request | `status`, `body_size_bytes`, `write_duration_ms` | Missing | `server/service.rs` |
| RF5 | Response complete | Critical | INFO | Per-request | `request_id`, `status`, `total_latency_ms`, `bytes_sent` | Partial | `server/service.rs:491` |

**Notes**:
- RF5 exists but lacks context (request_id, bytes_sent)
- RF2 is important for debugging serialization issues
- Currently using `println!` for RF5 instead of structured logging

### 2.9 Middleware Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| M1 | CORS headers added | Low | DEBUG | Per-request | `origin`, `allowed_methods[]`, `preflight` | Missing | `middleware/cors.rs` |
| M2 | CORS preflight handled | Medium | INFO | Per-preflight | `origin`, `requested_method`, `allowed` | Missing | `middleware/cors.rs` |
| M3 | Metrics recorded | Medium | DEBUG | Per-request | `metric_name`, `value`, `labels{}` | Missing | `middleware/metrics.rs` |
| M4 | Tracing span created | Low | DEBUG | Per-request | `span_id`, `trace_id`, `parent_span_id` | Partial | `middleware/tracing.rs:72` |
| M5 | Tracing span completed | Medium | INFO | Per-request | `span_id`, `duration_ms`, `events_count` | Partial | `middleware/tracing.rs:105` |
| M6 | Custom middleware error | High | ERROR | On-error | `middleware_name`, `error`, `request` (redacted) | Missing | Middleware implementations |

**Notes**:
- M4 and M5 are partially implemented but could be enhanced
- M6 is for user-defined middleware error handling
- Structured logging in middleware needs consistency

### 2.10 Startup Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| ST1 | Application start | High | INFO | Per-startup | `version`, `config_file`, `environment` | Missing | `main.rs` |
| ST2 | Config loaded | High | INFO | Per-startup | `config_file`, `overrides{}`, `validation_passed` | Missing | `main.rs` |
| ST3 | OpenAPI spec loaded | Critical | INFO | Per-startup | `spec_file`, `version`, `operations_count`, `schemas_count` | Missing | `spec/load.rs:53` |
| ST4 | Spec parsing failed | Critical | ERROR | On-error | `spec_file`, `error`, `line_number`, `validation_errors[]` | Missing | `spec/load.rs:55` |
| ST5 | Routes built | High | INFO | Per-startup | `routes_count`, `base_path`, `operations[]` | Partial | `router/core.rs:109` |
| ST6 | Handlers registered | High | INFO | Per-startup | `handlers_count`, `handlers[]` | Missing | `registry.rs` |
| ST7 | Security providers registered | Critical | INFO | Per-startup | `schemes[]`, `providers[]`, `config_source` | Missing | `main.rs / templates/main.rs.txt` |
| ST8 | Middleware initialized | Medium | INFO | Per-startup | `middleware[]`, `order[]` | Missing | `main.rs` |
| ST9 | Server listening | Critical | INFO | Per-startup | `bind_addr`, `port`, `protocol`, `tls_enabled` | Missing | `server/service.rs` |

**Notes**:
- ST3, ST4 are critical for debugging startup failures
- ST5 uses `println!` instead of structured logging
- ST7 is currently scattered across multiple println! statements
- ST9 is missing entirely but essential for confirming successful start

### 2.11 Hot Reload Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| HR1 | Spec file watch started | Medium | INFO | Per-startup | `watch_path`, `poll_interval_ms` | Missing | `hot_reload.rs` |
| HR2 | Spec file changed | Critical | INFO | On-change | `file_path`, `change_type`, `timestamp` | Missing | `hot_reload.rs` |
| HR3 | Spec reload initiated | High | INFO | On-reload | `old_routes_count`, `new_routes_count`, `handlers_affected[]` | Partial | `hot_reload.rs:110` |
| HR4 | Spec reload failed | Critical | ERROR | On-error | `error`, `validation_errors[]`, `rollback_applied` | Missing | `hot_reload.rs` |

**Notes**:
- HR3 has minimal logging with `info!` macro
- HR4 is critical for production hot reload safety
- Need to log which handlers are affected by reload

### 2.12 Code Generation Touchpoints

| # | Touchpoint | Criticality | Level | Frequency | Context Fields | Status | Location |
|---|------------|-------------|-------|-----------|----------------|--------|----------|
| CG1 | Generation started | High | INFO | Per-generation | `spec_file`, `output_dir`, `scope` (full/handlers/controllers) | Missing | `generator/project/generate.rs` |
| CG2 | Schema analysis | Medium | DEBUG | Per-generation | `schemas_count`, `types_generated`, `dependencies_resolved` | Missing | `generator/schema.rs` |
| CG3 | Template rendered | Low | DEBUG | Per-template | `template_name`, `output_file`, `lines_generated` | Missing | `generator/templates.rs` |
| CG4 | File written | Medium | INFO | Per-file | `file_path`, `size_bytes`, `overwrite` | Partial | `generator/project/generate.rs` |
| CG5 | Generation complete | High | INFO | Per-generation | `files_created`, `files_updated`, `total_lines`, `duration_ms` | Missing | `generator/project/generate.rs` |
| CG6 | Generation failed | Critical | ERROR | On-error | `error`, `phase`, `context`, `partial_files[]` | Missing | `generator/project/generate.rs` |
| CG7 | Rustfmt applied | Low | DEBUG | Per-generation | `files_formatted`, `rustfmt_version` | Missing | `generator/project/format.rs` |

**Notes**:
- CG4 has multiple `println!` statements that should be structured logs
- CG6 is critical for debugging generation failures
- CLI output should use proper logging levels

---

## 3. JSON Log Format Specification

### 3.1 Standard Fields

All logs MUST include these fields:

```json
{
  "timestamp": "2025-10-13T14:30:45.123Z",        // ISO8601 with milliseconds
  "level": "INFO",                                 // TRACE|DEBUG|INFO|WARN|ERROR
  "target": "brrtrouter::server::service",         // Rust module path
  "message": "Request completed successfully",     // Human-readable message
  "span": {                                        // OpenTelemetry span context
    "trace_id": "7f8d9e0a1b2c3d4e5f6g7h8i9j0k1l", // 32-char hex
    "span_id": "1234567890abcdef",                 // 16-char hex
    "parent_span_id": "fedcba0987654321"           // 16-char hex (optional)
  }
}
```

### 3.2 Request Context Fields

For logs within request handling, add these fields:

```json
{
  "request": {
    "id": "req_7f8d9e0a1b2c",                     // Generated request ID
    "method": "GET",                               // HTTP method
    "path": "/pets/123",                           // Request path
    "handler": "get_pet_by_id",                    // Handler name (if matched)
    "remote_addr": "203.0.113.42",                 // Client IP (may be masked)
    "http_version": "HTTP/1.1",                    // HTTP version
    "content_length": 1024,                        // Request body size in bytes
    "content_type": "application/json"             // Content-Type header
  }
}
```

### 3.3 Response Context Fields

For response-related logs:

```json
{
  "response": {
    "status": 200,                                 // HTTP status code
    "latency_ms": 45,                              // Total request latency
    "body_size": 2048,                             // Response body size in bytes
    "content_type": "application/json"             // Content-Type header
  }
}
```

### 3.4 Error Context Fields

For error logs (WARN, ERROR levels):

```json
{
  "error": {
    "type": "ValidationError",                     // Error type/class
    "message": "Invalid pet ID format",            // Error message
    "code": "E1001",                               // Optional error code
    "field": "id",                                 // Related field (if applicable)
    "details": {                                   // Additional error details
      "expected": "integer",
      "actual": "string"
    },
    "backtrace": "..."                             // Stack trace (ERROR level only)
  }
}
```

### 3.5 Component-Specific Fields

#### Security Context

```json
{
  "security": {
    "scheme": "BearerAuth",                        // Security scheme name
    "provider": "JwksBearerProvider",              // Provider type
    "scopes_required": ["read", "write"],          // Required scopes
    "scopes_granted": ["read"],                    // Granted scopes
    "user_id": "user_123",                         // User ID (if available)
    "reason": "insufficient_scope"                 // Failure reason
  }
}
```

#### Database Context

```json
{
  "database": {
    "operation": "SELECT",                         // Query type
    "table": "pets",                               // Table name
    "rows_affected": 1,                            // Rows returned/modified
    "duration_ms": 15,                             // Query duration
    "slow_query": false                            // Exceeded threshold?
  }
}
```

#### Dispatcher Context

```json
{
  "dispatcher": {
    "handler": "get_pet_by_id",                    // Handler name
    "channel_depth": 3,                            // Pending requests in channel
    "timeout": false,                              // Did request timeout?
    "panic": false                                 // Did handler panic?
  }
}
```

### 3.6 Example Logs

#### Example 1: Successful Request

```json
{
  "timestamp": "2025-10-13T14:30:45.123Z",
  "level": "INFO",
  "target": "brrtrouter::server::service",
  "message": "Request completed",
  "span": {
    "trace_id": "7f8d9e0a1b2c3d4e5f6g7h8i9j0k1l",
    "span_id": "1234567890abcdef"
  },
  "request": {
    "id": "req_7f8d9e0a1b2c",
    "method": "GET",
    "path": "/pets/123",
    "handler": "get_pet_by_id",
    "remote_addr": "203.0.113.42",
    "http_version": "HTTP/1.1"
  },
  "response": {
    "status": 200,
    "latency_ms": 45,
    "body_size": 2048,
    "content_type": "application/json"
  }
}
```

#### Example 2: Authentication Failure (401)

```json
{
  "timestamp": "2025-10-13T14:31:22.456Z",
  "level": "WARN",
  "target": "brrtrouter::server::service",
  "message": "Authentication failed: invalid token",
  "span": {
    "trace_id": "8g9h0i1j2k3l4m5n6o7p8q9r0s1t2u",
    "span_id": "2345678901bcdefg"
  },
  "request": {
    "id": "req_8g9h0i1j2k3l",
    "method": "GET",
    "path": "/pets",
    "handler": "get_pets",
    "remote_addr": "203.0.113.88"
  },
  "response": {
    "status": 401,
    "latency_ms": 2,
    "body_size": 128
  },
  "security": {
    "scheme": "BearerAuth",
    "provider": "JwksBearerProvider",
    "scopes_required": ["read"],
    "reason": "invalid_token"
  },
  "error": {
    "type": "AuthenticationError",
    "message": "JWT signature validation failed",
    "code": "E2001"
  }
}
```

#### Example 3: Handler Panic (500)

```json
{
  "timestamp": "2025-10-13T14:32:15.789Z",
  "level": "ERROR",
  "target": "brrtrouter::dispatcher::core",
  "message": "Handler panicked during execution",
  "span": {
    "trace_id": "9h0i1j2k3l4m5n6o7p8q9r0s1t2u3v",
    "span_id": "3456789012cdefgh"
  },
  "request": {
    "id": "req_9h0i1j2k3l4m",
    "method": "POST",
    "path": "/pets",
    "handler": "create_pet",
    "remote_addr": "203.0.113.15"
  },
  "response": {
    "status": 500,
    "latency_ms": 120,
    "body_size": 256
  },
  "dispatcher": {
    "handler": "create_pet",
    "panic": true
  },
  "error": {
    "type": "HandlerPanic",
    "message": "thread panicked at 'index out of bounds: the len is 0 but the index is 0'",
    "code": "E5001",
    "backtrace": "   0: rust_begin_unwind\n   1: core::panicking::panic_fmt\n   2: handlers::create_pet::create_pet\n   ..."
  }
}
```

#### Example 4: Validation Failure (400)

```json
{
  "timestamp": "2025-10-13T14:33:01.234Z",
  "level": "WARN",
  "target": "brrtrouter::server::service",
  "message": "Request validation failed",
  "span": {
    "trace_id": "0i1j2k3l4m5n6o7p8q9r0s1t2u3v4w",
    "span_id": "4567890123defghi"
  },
  "request": {
    "id": "req_0i1j2k3l4m5n",
    "method": "POST",
    "path": "/pets",
    "handler": "create_pet",
    "remote_addr": "203.0.113.99",
    "content_type": "application/json"
  },
  "response": {
    "status": 400,
    "latency_ms": 5,
    "body_size": 512
  },
  "error": {
    "type": "ValidationError",
    "message": "Request body does not match schema",
    "code": "E1002",
    "field": "age",
    "details": {
      "schema_path": "#/components/schemas/Pet",
      "errors": [
        {"field": "age", "expected": "integer", "actual": "string", "message": "Invalid type"}
      ]
    }
  }
}
```

#### Example 5: Route Not Found (404)

```json
{
  "timestamp": "2025-10-13T14:34:45.567Z",
  "level": "WARN",
  "target": "brrtrouter::server::service",
  "message": "No route matched",
  "span": {
    "trace_id": "1j2k3l4m5n6o7p8q9r0s1t2u3v4w5x",
    "span_id": "5678901234efghij"
  },
  "request": {
    "id": "req_1j2k3l4m5n6o",
    "method": "GET",
    "path": "/invalid/endpoint",
    "remote_addr": "203.0.113.77"
  },
  "response": {
    "status": 404,
    "latency_ms": 1,
    "body_size": 64
  },
  "error": {
    "type": "RouteNotFound",
    "message": "No route matched for GET /invalid/endpoint",
    "code": "E0001"
  }
}
```

### 3.7 Log Levels Usage Guide

| Level | When to Use | Examples |
|-------|-------------|----------|
| **TRACE** | Very detailed debugging (verbose) | Individual regex captures, buffer allocations |
| **DEBUG** | Development debugging info | Route matching attempts, parameter extraction, middleware execution |
| **INFO** | Normal operational events | Request completion, handler execution, startup events |
| **WARN** | Recoverable errors, degraded state | Auth failures (401/403), validation errors (400), rate limiting |
| **ERROR** | Critical errors requiring attention | Handler panics, JWKS fetch failures, spec parsing errors |

---

## 4. Sensitive Data Handling

### 4.1 Redaction Levels

Three redaction levels controlled by `BRRTR_LOG_REDACT_LEVEL`:

| Level | Description | Use Case |
|-------|-------------|----------|
| **none** | No redaction (DANGEROUS) | Local development only, never in production |
| **credentials** (default) | Redact API keys, tokens, passwords | Production default |
| **full** | Redact credentials + PII | GDPR/HIPAA compliance, regulated industries |

### 4.2 Always Redact (All Levels)

These fields MUST be redacted at ALL redaction levels:

#### Headers

- `Authorization` (completely redacted)
- `Cookie` (completely redacted)
- `X-API-Key` (truncate to first 4 chars + `***`)
- `Proxy-Authorization` (completely redacted)
- Any header containing: `token`, `secret`, `key`, `password`, `auth`

**Redaction Format**:
```json
{
  "headers": {
    "Authorization": "<REDACTED>",
    "X-API-Key": "test***",
    "Content-Type": "application/json"  // Safe header, not redacted
  }
}
```

#### Query Parameters

- `api_key`, `apikey`, `apiKey`
- `token`, `access_token`, `refresh_token`
- `secret`, `client_secret`
- `password`, `passwd`, `pwd`

**Redaction Format**:
```json
{
  "query_params": {
    "api_key": "abcd***",
    "limit": "10",      // Safe param, not redacted
    "offset": "0"       // Safe param, not redacted
  }
}
```

#### Request/Response Bodies

JSON fields containing sensitive keywords:

- `password`, `passwd`, `pwd`, `secret`
- `api_key`, `apiKey`, `apikey`
- `token`, `accessToken`, `refreshToken`
- `authorization`, `credentials`
- `ssn`, `social_security_number`
- `credit_card`, `creditCard`, `ccNumber`

**Redaction Format**:
```json
{
  "body": {
    "username": "john_doe",
    "password": "<REDACTED>",
    "email": "john@example.com",
    "api_key": "<REDACTED>"
  }
}
```

#### Cookies

All cookie values are redacted at `credentials` level:

```json
{
  "cookies": {
    "session": "<REDACTED>",
    "auth_token": "<REDACTED>",
    "preferences": "<REDACTED>"
  }
}
```

**Exception**: Cookie names (keys) are logged for debugging.

### 4.3 Full PII Redaction (Level: full)

At `full` redaction level, additionally redact:

#### Personal Identifiable Information

- **Email addresses**: Mask domain: `john***@example.com`
- **IP addresses**: Mask last octet: `203.0.113.*`
- **User IDs**: Hash or mask: `user_***` (keep prefix for correlation)
- **Phone numbers**: Mask all but country code: `+1-***-***-****`
- **Names**: Mask last name: `John D***`

**Example**:
```json
{
  "user": {
    "id": "user_***",
    "email": "john***@example.com",
    "name": "John D***",
    "ip_address": "203.0.113.*"
  }
}
```

### 4.4 Truncation Rules

For API keys and tokens, show **first 4 characters** for correlation:

```
Original:       test1234567890abcdef
Redacted:       test***

Original:       Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
Redacted:       Bear***
```

**Rationale**: First 4 chars allow correlating logs without exposing credentials.

### 4.5 Safe Headers Whitelist

These headers are **never redacted** (safe to log):

- `Accept`
- `Accept-Encoding`
- `Accept-Language`
- `Cache-Control`
- `Content-Length`
- `Content-Type`
- `Host`
- `Origin`
- `Referer` (may contain sensitive URLs - handle carefully)
- `User-Agent`
- `X-Request-ID`
- `X-Trace-ID`
- CORS headers (`Access-Control-*`)

### 4.6 Implementation Strategy

#### Option 1: Redaction Filter (Recommended)

Create a `tracing_subscriber` layer that filters logs before output:

```rust
pub struct RedactionLayer {
    level: RedactionLevel,
}

impl<S> Layer<S> for RedactionLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event, ctx: Context<S>) {
        // Intercept event, redact sensitive fields, pass to next layer
    }
}
```

#### Option 2: Redaction at Source

Redact at log call site:

```rust
info!(
    api_key = %redact_token(&api_key),
    "API key validation"
);
```

**Decision**: Use **Option 1** (RedactionLayer) for consistency and centralized control.

### 4.7 Redaction Testing

MUST test redaction with:

1. **Unit tests**: Verify regex patterns match all sensitive field names
2. **Integration tests**: Log sample requests, assert redaction applied
3. **Compliance audits**: Periodic review of production logs for leaks

**Test Cases**:
- ✅ API key truncated to 4 chars
- ✅ JWT tokens completely redacted
- ✅ Password fields masked in request bodies
- ✅ Cookie values redacted
- ✅ Email addresses masked at `full` level
- ✅ Safe headers (Content-Type) not redacted

---

## 5. Configuration Specification

### 5.1 Environment Variables

All logging configuration via environment variables for 12-factor app compliance:

| Variable | Type | Default | Values | Description |
|----------|------|---------|--------|-------------|
| `BRRTR_LOG_LEVEL` | String | `info` | `trace`, `debug`, `info`, `warn`, `error` | Minimum log level to emit |
| `BRRTR_LOG_FORMAT` | String | `json` | `json`, `pretty` | Log output format |
| `BRRTR_LOG_REDACT_LEVEL` | String | `credentials` | `none`, `credentials`, `full` | Redaction level for sensitive data |
| `BRRTR_LOG_SAMPLING_MODE` | String | `sampled` | `all`, `error-only`, `sampled` | Sampling strategy |
| `BRRTR_LOG_SAMPLING_RATE` | Float | `0.1` | `0.0` - `1.0` | Sampling rate (10% default) |
| `BRRTR_LOG_RATE_LIMIT_RPS` | Integer | `10` | `1` - `10000` | Max logs/sec per endpoint |
| `BRRTR_LOG_ASYNC` | Boolean | `true` | `true`, `false` | Enable async buffered logging |
| `BRRTR_LOG_BUFFER_SIZE` | Integer | `8192` | `512` - `65536` | Async buffer size (lines) |
| `BRRTR_LOG_TARGET_FILTER` | String | None | Comma-separated modules | Filter by module (e.g., `brrtrouter::server,brrtrouter::security`) |
| `BRRTR_LOG_INCLUDE_LOCATION` | Boolean | `false` | `true`, `false` | Include file:line in logs (dev only) |

### 5.2 Sampling Modes

#### Mode: `all`

Log every event (high volume):

```bash
BRRTR_LOG_SAMPLING_MODE=all
```

**Volume Estimate**: 40,000 req/s × 5 logs/req = 200,000 logs/sec

**Use Case**: Debugging production issues temporarily (< 5 minutes)

#### Mode: `error-only`

Log only WARN and ERROR levels:

```bash
BRRTR_LOG_SAMPLING_MODE=error-only
```

**Volume Estimate**: ~1% error rate = 400 logs/sec

**Use Case**: Production default for low-traffic services

#### Mode: `sampled` (Default)

Sample successful requests, log all errors:

```bash
BRRTR_LOG_SAMPLING_MODE=sampled
BRRTR_LOG_SAMPLING_RATE=0.1  # 10%
```

**Logic**:
- **ERROR/WARN**: Always logged (100%)
- **INFO/DEBUG**: Sampled at configured rate (10% default)
- **TRACE**: Sampled at 1/10th of INFO rate (1% default)

**Volume Estimate**: 40,000 req/s × 0.1 × 5 logs/req + 400 errors/sec = ~20,400 logs/sec

**Use Case**: Production default for high-traffic services

### 5.3 Rate Limiting

Prevent log flooding from single endpoint:

```bash
BRRTR_LOG_RATE_LIMIT_RPS=10
```

**Implementation**: Token bucket per `(level, target, message_template)`.

**Example**:
```rust
// First 10 logs per second are emitted
info!("Request completed");
// After 10/sec, additional logs are dropped (sampled at 1/100)
```

**Drop Counter**: Track dropped logs and emit periodic summary:
```json
{
  "level": "WARN",
  "message": "Rate limit: dropped 150 logs in last second",
  "target": "brrtrouter::server::service",
  "dropped_count": 150,
  "rate_limit_rps": 10
}
```

### 5.4 Async Buffering

Enable async logging for minimal latency impact:

```bash
BRRTR_LOG_ASYNC=true
BRRTR_LOG_BUFFER_SIZE=8192
```

**Architecture**:
```
Request → Log Event → Async Channel → Background Thread → Stdout/Loki
           (instant)   (buffered)       (batched I/O)
```

**Benefits**:
- <1µs logging latency (vs ~50-100µs synchronous)
- Batched I/O reduces syscalls
- Non-blocking for request threads

**Trade-offs**:
- Logs may be lost on crash (buffer not flushed)
- Increased memory usage (buffer size × log size)

**Buffer Sizing**:
- **8192** (default): ~8MB RAM for 1KB logs = 8 seconds at 1000 logs/sec
- **16384** (high-traffic): ~16MB RAM = 16 seconds buffer
- **4096** (low-memory): ~4MB RAM = 4 seconds buffer

### 5.5 Development vs Production Presets

#### Development

```bash
export BRRTR_LOG_LEVEL=debug
export BRRTR_LOG_FORMAT=pretty
export BRRTR_LOG_REDACT_LEVEL=none
export BRRTR_LOG_SAMPLING_MODE=all
export BRRTR_LOG_ASYNC=false
export BRRTR_LOG_INCLUDE_LOCATION=true
```

**Output** (pretty format):
```
2025-10-13 14:30:45.123 DEBUG brrtrouter::server::service
    Request received method=GET path=/pets/123 handler=get_pet_by_id
    at src/server/service.rs:532
```

#### Production

```bash
export BRRTR_LOG_LEVEL=info
export BRRTR_LOG_FORMAT=json
export BRRTR_LOG_REDACT_LEVEL=credentials
export BRRTR_LOG_SAMPLING_MODE=sampled
export BRRTR_LOG_SAMPLING_RATE=0.1
export BRRTR_LOG_RATE_LIMIT_RPS=10
export BRRTR_LOG_ASYNC=true
export BRRTR_LOG_BUFFER_SIZE=8192
export BRRTR_LOG_INCLUDE_LOCATION=false
```

**Output** (JSON format):
```json
{"timestamp":"2025-10-13T14:30:45.123Z","level":"INFO","target":"brrtrouter::server::service","message":"Request received","method":"GET","path":"/pets/123","handler":"get_pet_by_id"}
```

### 5.6 Dynamic Configuration (Future)

Consider adding runtime configuration via:
- HTTP endpoint: `POST /admin/logging/config`
- Signal handler: `SIGUSR1` to toggle debug logging
- Config file hot-reload: Watch `config/logging.yaml`

**Not in scope for v1**, but document for future enhancement.

---

## 6. Implementation Guidance

### 6.1 Tracing Crate Architecture

#### Core Components

```rust
use tracing::{info, warn, error, debug, trace, info_span, Instrument};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Registry
};
use tracing_subscriber::fmt::layer as fmt_layer;
```

**Layers Architecture**:
```
Application Log Events
         ↓
tracing::info!(...)
         ↓
tracing_subscriber::Registry
         ↓
    ┌────┴────┐
    ↓         ↓
EnvFilter  RedactionLayer
    ↓         ↓
FmtLayer(JSON)
    ↓
 Stdout/Loki
```

#### Initialization

```rust
pub fn init_logging(config: &LogConfig) -> anyhow::Result<()> {
    let env_filter = EnvFilter::new(&config.log_level);
    
    let fmt_layer = if config.format == "json" {
        fmt_layer().json()
            .with_current_span(true)
            .with_span_list(true)
            .with_target(true)
            .with_thread_ids(config.include_location)
    } else {
        fmt_layer().pretty()
    };
    
    let redaction_layer = RedactionLayer::new(config.redact_level);
    let sampling_layer = SamplingLayer::new(config.sampling_mode, config.sampling_rate);
    
    if config.async_logging {
        let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
        Registry::default()
            .with(env_filter)
            .with(redaction_layer)
            .with(sampling_layer)
            .with(fmt_layer.with_writer(non_blocking))
            .try_init()?;
    } else {
        Registry::default()
            .with(env_filter)
            .with(redaction_layer)
            .with(sampling_layer)
            .with(fmt_layer)
            .try_init()?;
    }
    
    Ok(())
}
```

### 6.2 Spans vs Events

#### Use Spans For

- **Request lifecycle**: `info_span!("http_request")`
- **Handler execution**: `info_span!("handler_execute")`
- **Database queries**: `debug_span!("db_query")`
- **External API calls**: `info_span!("api_call")`

**Example**:
```rust
let span = info_span!(
    "http_request",
    method = %method,
    path = %path,
    request_id = %request_id
);

async {
    // ... request handling ...
}.instrument(span).await;
```

#### Use Events For

- **Specific log points**: Authentication success/failure
- **Errors**: Validation failures, panics
- **Metrics**: Request counts, latencies
- **State changes**: Hot reload applied, handler registered

**Example**:
```rust
info!(
    method = %method,
    path = %path,
    status = response.status,
    latency_ms = latency.as_millis(),
    "Request completed"
);
```

### 6.3 Structured Fields

Use structured fields (not string interpolation):

❌ **Bad**:
```rust
info!("Request completed: {} {} -> {}", method, path, status);
```

✅ **Good**:
```rust
info!(
    method = %method,
    path = %path,
    status = response.status,
    "Request completed"
);
```

**Field Formatters**:
- `%field`: Display formatting (`std::fmt::Display`)
- `?field`: Debug formatting (`std::fmt::Debug`)
- `field`: Direct value (must implement `tracing::Value`)

### 6.4 Request ID Generation

Generate unique request ID at request entry (ULID, echoed via X-Request-ID):

```rust
use brrtrouter::ids::RequestId;

let request_id = RequestId::new();
// Example: "01HV7Z6K5S9R0J6M0F1Q2W3E4R"
```

**Propagate** through:
- `HandlerRequest` struct (`request_id: RequestId`)
- All log events within request span
- HTTP response header: `X-Request-ID`

### 6.5 Error Context

For errors, include full context:

```rust
match result {
    Ok(pet) => { /* ... */ },
    Err(e) => {
        error!(
            error_type = %std::any::type_name_of_val(&e),
            error_message = %e,
            handler = %handler_name,
            pet_id = pet_id,
            "Failed to fetch pet from database"
        );
        // Return 500 response
    }
}
```

For panics (in dispatcher catch_unwind):

```rust
if let Err(panic) = std::panic::catch_unwind(|| { ... }) {
    error!(
        handler = %handler_name,
        panic_message = ?panic,
        backtrace = %std::backtrace::Backtrace::capture(),
        "Handler panicked"
    );
}
```

### 6.6 Migration Path

#### Phase 1: Core Infrastructure (Week 1)

1. Implement `RedactionLayer` and `SamplingLayer`
2. Update `otel::init_logging()` with new config
3. Add `LogConfig` struct with env var parsing
4. Write unit tests for redaction and sampling

#### Phase 2: Server & Routing (Week 2)

5. Add logging to `server/service.rs` (all touchpoints from table)
6. Add logging to `router/core.rs` (route matching)
7. Add request ID generation and propagation
8. Test with sample requests

#### Phase 3: Security & Validation (Week 3)

9. Add logging to security validation (all touchpoints)
10. Add logging to request/response validation
11. Test with auth failures and validation errors

#### Phase 4: Dispatcher & Handlers (Week 4)

12. Add logging to dispatcher (all touchpoints)
13. Update handler templates with logging examples
14. Add panic logging with backtraces
15. Test with handler panics and timeouts

#### Phase 5: Startup & Hot Reload (Week 5)

16. Add logging to startup sequence
17. Add logging to hot reload
18. Add logging to code generator (CLI commands)
19. Integration testing

#### Phase 6: Polish & Documentation (Week 6)

20. Replace all `println!`/`eprintln!` with `tracing` macros
21. Performance testing (latency impact)
22. Update documentation with logging examples
23. Create runbook for log analysis

### 6.7 Testing Strategy

#### Unit Tests

Test individual logging components:

```rust
#[test]
fn test_redaction_api_key() {
    let redactor = Redactor::new(RedactionLevel::Credentials);
    let input = json!({"api_key": "test1234567890"});
    let output = redactor.redact_json(&input);
    assert_eq!(output["api_key"], "test***");
}
```

#### Integration Tests

Test full request logging:

```rust
#[tokio::test]
async fn test_request_logging() {
    let _guard = init_test_logging();
    
    let response = client.get("/pets/123")
        .header("X-API-Key", "test123")
        .send()
        .await?;
    
    // Assert logs were emitted (check test capture)
    assert_log_contains("Request completed");
    assert_log_field("status", 200);
    assert_log_field("api_key", "test***");
}
```

#### Log Capture for Tests

Use `tracing-subscriber::fmt::test()`:

```rust
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;

fn init_test_logging() -> impl Drop {
    let subscriber = tracing_subscriber::registry()
        .with(layer().with_test_writer());
    tracing::subscriber::set_default(subscriber)
}
```

### 6.8 Loki Integration

#### Label Strategy

Loki uses labels for indexing (limit to high-cardinality):

**Good Labels** (low cardinality):
- `app="brrtrouter"`
- `level="info"`
- `environment="production"`
- `handler="get_pet_by_id"` (if <100 handlers)

**Bad Labels** (high cardinality):
- ❌ `request_id` (unique per request)
- ❌ `path` (too many variations)
- ❌ `user_id` (high cardinality)

**Best Practice**: Use labels for filtering, put details in JSON fields.

#### Promtail Configuration

```yaml
server:
  http_listen_port: 9080

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: brrtrouter
    static_configs:
      - targets:
          - localhost
        labels:
          app: brrtrouter
          environment: production
          __path__: /var/log/brrtrouter/*.log
    pipeline_stages:
      - json:
          expressions:
            level: level
            timestamp: timestamp
            target: target
      - labels:
          level:
          target:
      - timestamp:
          source: timestamp
          format: RFC3339
```

#### LogQL Queries

```logql
# All errors in last hour
{app="brrtrouter", level="ERROR"} |~ ".*"

# Auth failures for specific handler
{app="brrtrouter"} | json | handler="get_pets" | security_reason="invalid_token"

# Slow requests (>1s latency)
{app="brrtrouter"} | json | latency_ms > 1000

# Panic rate (per minute)
rate({app="brrtrouter"} | json | error_type="HandlerPanic" [1m])
```

### 6.9 Performance Considerations

#### Benchmarking

Test latency impact:

```rust
#[bench]
fn bench_request_with_logging(b: &mut Bencher) {
    init_async_logging();
    b.iter(|| {
        info!(method = "GET", path = "/test", "Request completed");
    });
}

#[bench]
fn bench_request_without_logging(b: &mut Bencher) {
    b.iter(|| {
        // No logging
    });
}
```

**Target**: Async logging <1µs overhead per log call.

#### Async Buffer Tuning

Monitor buffer depth:

```rust
let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());
let buffer_depth = guard.buffer_len(); // Monitor in metrics
```

**Alert** if buffer depth consistently >80% (risk of drops).

#### Sampling Impact

Sampling reduces log volume but may miss issues:

- **10% sampling**: 90% of successful requests not logged
- **Error-only**: All successes not logged

**Mitigation**: Use distributed tracing (Jaeger) for request visibility.

---

## 7. Success Metrics

### 7.1 Coverage Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Critical touchpoints logged** | 100% (22/22) | Manual audit of code vs touchpoint table |
| **High touchpoints logged** | 100% (29/29) | Manual audit of code vs touchpoint table |
| **Medium touchpoints logged** | ≥80% (16/20) | Manual audit of code vs touchpoint table |
| **Redaction test coverage** | 100% | Unit tests for all sensitive field patterns |

### 7.2 Performance Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Latency overhead (async)** | <5% | Benchmark with/without logging (p50, p99) |
| **Latency overhead (sync)** | <10% | Benchmark with/without logging (sync mode) |
| **CPU overhead** | <2% | CPU profiling under load (flamegraph) |
| **Memory overhead** | <50MB | Buffer size + subscriber state |
| **Log volume (sampled)** | <1000 logs/sec | Prometheus counter at 40k req/s |

### 7.3 Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Zero credential leaks** | 100% | Weekly audit of sample production logs |
| **JSON parse success rate** | >99.9% | Loki ingestion error rate |
| **Log completeness (fields)** | >95% | Random sample: all expected fields present |
| **Consistent message format** | >95% | Lint logs for format consistency |

### 7.4 Operational Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Mean time to debug (MTTD)** | -50% | Track incident resolution time (before/after) |
| **Incidents requiring SSH** | -70% | Count incidents resolved via logs vs SSH |
| **False positive alerts** | <5% | Alerts triggered by log anomalies |
| **Log storage cost** | <$100/month | Loki storage costs (estimate 1TB/month at 10% sampling) |

### 7.5 Adoption Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Generated handlers with logging** | 100% | Template audit |
| **Developer satisfaction** | ≥4/5 | Survey: "Logging helps me debug issues" |
| **Documentation completeness** | 100% | All touchpoints documented with examples |

---

## 8. Open Questions

### 8.1 Database Query Logging

**Question**: Should we log all database queries or only slow queries?

**Options**:
- **A**: Log all queries at DEBUG level (high volume)
- **B**: Log only slow queries (>100ms) at WARN level
- **C**: Sample queries at 10% at DEBUG level
- **D**: Log query counts/latency in aggregate (metrics only)

**Recommendation**: **Option C** (sampled) + **Option D** (aggregate metrics)

**Rationale**: Individual query logs help debug specific issues; aggregate metrics track overall health.

**Implementation**: Add `database` span with query details (redact sensitive WHERE clauses).

### 8.2 Distributed Tracing Correlation

**Question**: How do we correlate BRRTRouter logs with external services (database, APIs)?

**Options**:
- **A**: Inject trace context into database queries (SQLCommenter)
- **B**: Pass trace ID in HTTP headers to external APIs (W3C Trace Context)
- **C**: Log external request/response for manual correlation
- **D**: Rely on OpenTelemetry tracing only (no log correlation)

**Recommendation**: **Option B** (W3C Trace Context) + **Option C** (log external calls)

**Implementation**: 
- Extract `traceparent` header from requests
- Inject `traceparent` header into external HTTP calls
- Log external API calls with trace ID

### 8.3 Log Retention Policy

**Question**: How long should logs be retained?

**Current State**: Undefined (Loki default: unlimited until disk full)

**Recommendation**:
- **Hot storage** (Loki): 7 days (fast query)
- **Warm storage** (S3): 30 days (archived, slower query)
- **Cold storage** (Glacier): 90 days (compliance)

**Volume Estimates** (at 40k req/s, 10% sampling, 1KB/log):
- Per day: 20,400 logs/sec × 86,400 sec × 1KB = ~1.76 TB/day
- 7 days (hot): ~12 TB
- 30 days (warm): ~53 TB
- 90 days (cold): ~158 TB

**Cost Estimate** (S3 pricing):
- Hot (Loki): $0.023/GB/month × 12,000 GB = **$276/month**
- Warm (S3 Standard): $0.023/GB/month × 53,000 GB = **$1,219/month**
- Cold (Glacier): $0.004/GB/month × 158,000 GB = **$632/month**
- **Total: ~$2,127/month**

**Mitigation**: Increase sampling rate (1% instead of 10%) to reduce volume 10×.

### 8.4 Error Rate Thresholds

**Question**: What error rates should trigger alerts?

**Recommendation**:
- **Critical (PagerDuty)**: >5% error rate sustained for 5 minutes
- **Warning (Slack)**: >1% error rate sustained for 10 minutes
- **Info**: Individual ERROR logs (for review)

**Specific Thresholds**:
- Auth failures (401): >10% of requests (potential attack)
- Handler panics (500): >0.1% of requests (critical bug)
- Validation failures (400): >20% of requests (client issue or API change)
- JWKS fetch failures: Any failure (critical auth issue)

### 8.5 Log Sampling Configuration UI

**Question**: Should we provide a UI to adjust log sampling dynamically?

**Current**: Environment variables (requires restart)

**Future Enhancement**: 
- Admin API: `POST /admin/logging/config`
- Temporary debug mode: Enable DEBUG logging for specific user/session
- Per-handler sampling: Different rates for critical vs non-critical handlers

**Out of scope for v1**, but document for future consideration.

---

## 9. Appendices

### 9.1 Glossary

| Term | Definition |
|------|------------|
| **Span** | A unit of work in distributed tracing (e.g., HTTP request, database query) |
| **Event** | A point-in-time log message within a span |
| **Target** | Rust module path (e.g., `brrtrouter::server::service`) |
| **Field** | Structured key-value pair in a log (e.g., `method = "GET"`) |
| **Redaction** | Masking sensitive data in logs (e.g., API keys → `test***`) |
| **Sampling** | Logging only a percentage of events to reduce volume |
| **Rate Limiting** | Dropping logs exceeding a frequency threshold (e.g., 10/sec) |
| **Async Logging** | Buffering logs in memory and writing asynchronously |
| **Loki** | Log aggregation system by Grafana Labs |
| **LogQL** | Query language for Loki (similar to PromQL) |

### 9.2 Related Documentation

- [Request Lifecycle](../RequestLifecycle.md) - End-to-end request flow
- [Security & Authentication](../SecurityAuthentication.md) - Security provider details
- [Architecture](../ARCHITECTURE.md) - System design overview
- [Observability Stack](./OBSERVABILITY_COMPLETE.md) - Prometheus/Grafana/Loki setup

### 9.3 References

- [Tracing Crate Documentation](https://docs.rs/tracing/)
- [Tracing Subscriber](https://docs.rs/tracing-subscriber/)
- [OpenTelemetry Specification](https://opentelemetry.io/docs/specs/otel/)
- [Loki Documentation](https://grafana.com/docs/loki/)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
- [12-Factor App Logs](https://12factor.net/logs)

### 9.4 Change Log

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-10-13 | System Architecture | Initial PRD |

---

## Summary

This PRD defines comprehensive logging requirements for BRRTRouter, covering:

- ✅ **78 logging touchpoints** across all system components
- ✅ **Structured JSON format** with standard fields and examples
- ✅ **Sensitive data redaction** with three levels (none/credentials/full)
- ✅ **Flexible configuration** via 10 environment variables
- ✅ **Performance optimizations** (async buffering, sampling, rate limiting)
- ✅ **Implementation guidance** (spans vs events, migration path, testing)
- ✅ **Success metrics** (coverage, performance, quality, operational)
- ✅ **Open questions** for stakeholder decision

**Next Steps**:
1. Review and approve PRD with stakeholders
2. Prioritize touchpoints (Critical → High → Medium → Low)
3. Begin Phase 1 implementation (Core Infrastructure)
4. Iterate based on feedback and metrics

**Estimated Implementation**: 6 weeks (full-time engineer)

---

**Document End**

