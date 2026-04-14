# Log Analysis Runbook

**Version**: 1.0  
**Last Updated**: October 2025  
**Audience**: Developers, SREs, Operations Teams

This runbook provides practical guidance for analyzing BRRTRouter logs to debug issues, troubleshoot production incidents, and monitor system health.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Log Structure](#log-structure)
- [Common Scenarios](#common-scenarios)
- [Loki Queries](#loki-queries)
- [Performance Analysis](#performance-analysis)
- [Security Incident Response](#security-incident-response)
- [Troubleshooting Patterns](#troubleshooting-patterns)

---

## Quick Start

### Access Logs

**Development (stdout)**:
```bash
# Pretty-printed logs
BRRTR_LOG_FORMAT=pretty cargo run

# JSON logs
BRRTR_LOG_FORMAT=json cargo run
```

**Production (Loki)**:
```bash
# Via Grafana UI
http://localhost:3000/explore

# Via LogCLI
logcli query '{job="brrtrouter"} | json'
```

### Essential Log Fields

| Field | Description | Example |
|-------|-------------|---------|
| `timestamp` | ISO8601 timestamp | `2025-10-13T12:34:56.789Z` |
| `level` | Log level (TRACE/DEBUG/INFO/WARN/ERROR) | `INFO` |
| `target` | Module path | `brrtrouter::dispatcher::core` |
| `message` | Human-readable message | `Request dispatched to handler` |
| `request_id` | Unique request identifier (ULID) | `01HV7Z6K5S9R0J6M0F1Q2W3E4R` |
| `handler_name` | Handler function name | `get_pet_by_id` |
| `method` | HTTP method | `GET` |
| `path` | Request path | `/pets/123` |
| `status` | HTTP status code | `200` |
| `latency_ms` | Request duration in milliseconds | `42` |

---

## Log Structure

### Standard JSON Log Entry

```json
{
  "timestamp": "2025-10-13T12:34:56.789Z",
  "level": "INFO",
  "target": "brrtrouter::dispatcher::core",
  "message": "Request dispatched to handler",
  "request_id": "01HV7Z6K5S9R0J6M0F1Q2W3E4R",
  "handler_name": "get_pet_by_id",
  "method": "GET",
  "path": "/pets/123",
  "span": {
    "trace_id": "0af7651916cd43dd8448eb211c80319c",
    "span_id": "00f067aa0ba902b7"
  }
}
```

### Log Levels

| Level | Usage | Volume (typical) |
|-------|-------|------------------|
| **ERROR** | Critical failures, panics | <0.01% |
| **WARN** | Auth failures, validation errors, retryable failures | 0.1-1% |
| **INFO** | Request lifecycle, route matches, handler execution | 10-20% |
| **DEBUG** | Headers, params, detailed flow | 80-90% |
| **TRACE** | Very verbose, rarely used | Disabled in production |

---

## Common Scenarios

### 1. Tracing a Single Request

**Problem**: A specific request failed. Trace its entire lifecycle.

**Solution**: Use `request_id` (ULID) to correlate all logs for that request.

**Loki Query**:
```logql
{job="brrtrouter"} | json | request_id="01HV7Z6K5S9R0J6M0F1Q2W3E4R"
```

**Expected Log Sequence**:
1. `HTTP request parsed` (INFO) - Request entry
2. `Route match attempt` (DEBUG) - Router lookup
3. `Route matched` (INFO) - Handler identified
4. `Security check start` (DEBUG) - Auth begins
5. `Authentication success` (INFO) - Auth passed
6. `Request validation start` (DEBUG) - Schema validation
7. `Request dispatched to handler` (INFO) - Sent to handler
8. `Handler execution start` (INFO) - Handler begins
9. `Handler execution complete` (INFO) - Handler done
10. `Handler response received` (INFO) - Response ready

### 2. Debugging 404 Errors

**Problem**: Users hitting 404s unexpectedly.

**Solution**: Find all route match failures and analyze attempted patterns.

**Loki Query**:
```logql
{job="brrtrouter"} | json | message="No route matched" | line_format "{{.method}} {{.path}} tried {{.attempted_patterns}}"
```

**Analysis Checklist**:
- [ ] Is the path pattern correct in OpenAPI spec?
- [ ] Are path parameters formatted correctly? (e.g., `{id}` vs `{petId}`)
- [ ] Is the HTTP method supported for this path?
- [ ] Has hot reload completed successfully?

### 3. Investigating Authentication Failures

**Problem**: Users reporting 401/403 errors.

**Solution**: Analyze security logs to identify failure patterns.

**Loki Query (401s)**:
```logql
{job="brrtrouter"} | json | message="Authentication failed (401 unauthorized)"
```

**Loki Query (403s)**:
```logql
{job="brrtrouter"} | json | message="Insufficient scope (403 forbidden)"
```

**Analysis**:
```json
// Example 401 log
{
  "message": "Authentication failed (401 unauthorized)",
  "method": "GET",
  "path": "/pets",
  "handler": "get_pets",
  "status": 401,
  "reason": "invalid_credentials",
  "schemes_required": ["BearerAuth"],
  "attempted_schemes": ["BearerAuth"]
}

// Example 403 log
{
  "message": "Insufficient scope (403 forbidden)",
  "method": "POST",
  "path": "/pets",
  "handler": "create_pet",
  "status": 403,
  "reason": "insufficient_scope",
  "schemes_required": ["BearerAuth"],
  "scopes_required": ["write"],
  "attempted_schemes": ["BearerAuth"]
}
```

**Troubleshooting Steps**:
1. **401**: Check if token is expired, malformed, or missing
2. **403**: Verify user has required scopes in JWT claims
3. Check `attempted_schemes` matches `schemes_required`
4. Verify security provider is registered correctly

### 4. Analyzing Handler Panics

**Problem**: Handlers crashing with 500 errors.

**Solution**: Search for panic logs with full backtraces.

**Loki Query**:
```logql
{job="brrtrouter"} | json | message="Handler panicked - CRITICAL"
```

**Example Panic Log**:
```json
{
  "level": "ERROR",
  "message": "Handler panicked - CRITICAL",
  "request_id": "01HV7Z6K5S9R0J6M0F1Q2W3E4R",
  "handler_name": "divide_by_zero_handler",
  "panic_message": "attempt to divide by zero",
  "backtrace": "   0: std::backtrace::Backtrace::capture\n   1: brrtrouter::dispatcher::core::register_handler::{{closure}}\n   2: may::coroutine::Coroutine::run\n   ..."
}
```

**Recovery Steps**:
1. Identify handler from `handler_name` field
2. Review backtrace to find exact line
3. Check if panic is data-dependent (use `request_id` to review request params)
4. Deploy hotfix or add input validation

### 5. Monitoring Hot Reload Events

**Problem**: Spec changes not taking effect or causing issues.

**Solution**: Review hot reload logs for success/failure.

**Loki Query (Success)**:
```logql
{job="brrtrouter"} | json | message="Spec reload success"
```

**Loki Query (Failure)**:
```logql
{job="brrtrouter"} | json | message="Spec reload failed"
```

**Example Success Log**:
```json
{
  "level": "INFO",
  "message": "Spec reload success",
  "spec_path": "examples/openapi.yaml",
  "routes_count": 12,
  "reload_time_ms": 45,
  "routes": ["GET /pets", "POST /pets", "GET /pets/{id}", "..."]
}
```

**Example Failure Log**:
```json
{
  "level": "ERROR",
  "message": "Spec reload failed",
  "spec_path": "examples/openapi.yaml",
  "reload_time_ms": 23,
  "error": "YAML parse error: invalid syntax at line 42",
  "error_type": "serde_yaml::error::Error"
}
```

---

## Loki Queries

### Performance Queries

**Slowest Requests (>1s latency)**:
```logql
{job="brrtrouter"} | json | latency_ms > 1000 | line_format "{{.handler_name}} took {{.latency_ms}}ms"
```

**Average Latency by Handler**:
```logql
avg_over_time({job="brrtrouter"} | json | unwrap latency_ms [5m]) by (handler_name)
```

**Request Volume by Endpoint**:
```logql
sum(rate({job="brrtrouter"} | json | message="Request dispatched to handler" [5m])) by (handler_name)
```

### Error Queries

**All Errors (Last 1 Hour)**:
```logql
{job="brrtrouter"} | json | level="ERROR" | line_format "{{.timestamp}} {{.message}}"
```

**Error Rate by Handler**:
```logql
sum(rate({job="brrtrouter"} | json | level="ERROR" [5m])) by (handler_name)
```

**Top 10 Error Messages**:
```logql
topk(10, sum(count_over_time({job="brrtrouter"} | json | level="ERROR" [1h])) by (message))
```

### Security Queries

**All Auth Failures**:
```logql
{job="brrtrouter"} | json | status=~"401|403"
```

**Failed Logins by Path**:
```logql
sum(count_over_time({job="brrtrouter"} | json | status="401" [1h])) by (path)
```

**Brute Force Detection (>10 401s in 1min)**:
```logql
sum(rate({job="brrtrouter"} | json | status="401" [1m])) by (path) > 10
```

---

## Performance Analysis

### Identifying Bottlenecks

**1. Handler Execution Time**

Find handlers with high execution time:

```logql
{job="brrtrouter"} | json | message="Handler execution complete" | execution_time_ms > 100
```

**2. Dispatcher Latency**

Compare `Handler response received` latency_ms with `Handler execution complete` execution_time_ms:

```logql
{job="brrtrouter"} | json | message=~"Handler response received|Handler execution complete" | request_id="01*"
```

**3. Validation Overhead**

Time spent in request validation:

```logql
{job="brrtrouter"} | json | message=~"Request validation start|Request dispatched to handler" | request_id="01*"

### Correlation ID behavior

- Ingress: server accepts `X-Request-ID`. If present and valid as ULID (or supported formats), it is used; otherwise, a new ULID is generated.
- Egress: server always echoes `X-Request-ID` in responses.
- Logging: all structured logs include `request_id` as a ULID string.
- Metrics: `request_id` is intentionally never used as a Prometheus label.
```

### Latency Percentiles

**P50, P90, P99 Latency**:
```logql
quantile_over_time(0.50, {job="brrtrouter"} | json | unwrap latency_ms [5m]) by (handler_name)
quantile_over_time(0.90, {job="brrtrouter"} | json | unwrap latency_ms [5m]) by (handler_name)
quantile_over_time(0.99, {job="brrtrouter"} | json | unwrap latency_ms [5m]) by (handler_name)
```

---

## Security Incident Response

### Incident Types

#### 1. Brute Force Attack

**Detection**:
```logql
sum(rate({job="brrtrouter"} | json | status="401" [1m])) by (path) > 10
```

**Response**:
1. Identify attacking IPs (requires reverse proxy logs)
2. Review `attempted_schemes` - is attacker trying multiple auth methods?
3. Temporarily increase JWT expiry checks
4. Consider rate limiting at reverse proxy

#### 2. Token Theft / Replay Attack

**Detection**:
```logql
{job="brrtrouter"} | json | message="Authentication success" | handler_name="high_value_handler"
```

**Response**:
1. Review geographic patterns (requires IP geolocation)
2. Check for rapid token reuse across different handlers
3. Verify JWT `jti` (nonce) is properly checked
4. Consider short-lived tokens + refresh tokens

#### 3. Privilege Escalation Attempts

**Detection**:
```logql
{job="brrtrouter"} | json | status="403" | message="Insufficient scope"
```

**Response**:
1. Identify users hitting 403s repeatedly
2. Review `scopes_required` vs user's actual scopes
3. Check for attempts to access admin-only endpoints
4. Audit role assignment process

---

## Troubleshooting Patterns

### Pattern 1: Intermittent 500 Errors

**Symptoms**: Occasional 500s, not reproducible.

**Investigation Steps**:
1. Find all ERROR logs in timeframe:
   ```logql
   {job="brrtrouter"} | json | level="ERROR"
   ```
2. Check for handler panics:
   ```logql
   {job="brrtrouter"} | json | message="Handler panicked"
   ```
3. Review handler timeout logs:
   ```logql
   {job="brrtrouter"} | json | message="Handler timeout or channel closed"
   ```
4. Correlate with resource usage (CPU/memory spikes)

### Pattern 2: Gradual Performance Degradation

**Symptoms**: Latency increases over time.

**Investigation Steps**:
1. Plot latency over time:
   ```logql
   avg_over_time({job="brrtrouter"} | json | unwrap latency_ms [5m])
   ```
2. Check for memory leaks (requires external monitoring)
3. Review coroutine pool exhaustion:
   ```logql
   {job="brrtrouter"} | json | message="Handler timeout"
   ```
4. Analyze slow handlers:
   ```logql
   {job="brrtrouter"} | json | execution_time_ms > 1000
   ```

### Pattern 3: Validation Failures After Spec Change

**Symptoms**: Sudden spike in 400 errors after hot reload.

**Investigation Steps**:
1. Review hot reload event:
   ```logql
   {job="brrtrouter"} | json | message="Spec reload success"
   ```
2. Find validation failures:
   ```logql
   {job="brrtrouter"} | json | message="Request schema validation failed"
   ```
3. Review `invalid_fields` to identify new required fields:
   ```json
   {
     "message": "Request schema validation failed",
     "errors": ["'age' is required"],
     "schema_path": "#/components/schemas/request",
     "invalid_fields": ["age"]
   }
   ```
4. Communicate API changes to clients or revert spec

### Pattern 4: Security Provider Not Found

**Symptoms**: All requests return 500, logs show "Security provider not found".

**Investigation Steps**:
1. Find provider lookup failures:
   ```logql
   {job="brrtrouter"} | json | message="Security provider not found"
   ```
2. Review `available_providers` vs `scheme_name` required:
   ```json
   {
     "message": "Security provider not found",
     "scheme_name": "OAuth2Bearer",
     "available_providers": ["BearerAuth", "ApiKeyAuth"]
   }
   ```
3. Check OpenAPI spec: Is `OAuth2Bearer` defined in `components.securitySchemes`?
4. Verify security provider registration in `main.rs`

---

## Additional Resources

- [Logging PRD](../docs/wip/LOGGING_PRD.md) - Complete touchpoint documentation
- [Architecture Docs](./ARCHITECTURE.md#observability--logging) - Logging architecture overview
- [Security Documentation](./SecurityAuthentication.md) - Security provider details
- [Grafana Loki Docs](https://grafana.com/docs/loki/latest/) - Query language reference

---

## Quick Reference: Log Messages

| Message | Level | Component | Meaning |
|---------|-------|-----------|---------|
| `HTTP request parsed` | INFO | server/request | Request entry point |
| `Route matched` | INFO | router/core | Handler identified |
| `No route matched` | WARN | router/core | 404 error |
| `Security check start` | DEBUG | server/service | Auth beginning |
| `Authentication success` | INFO | server/service | Auth passed |
| `Authentication failed (401)` | WARN | server/service | Invalid credentials |
| `Insufficient scope (403)` | WARN | server/service | Missing permissions |
| `Request validation start` | DEBUG | server/service | Schema validation |
| `Request schema validation failed` | WARN | server/service | 400 error |
| `Request dispatched to handler` | INFO | dispatcher/core | Sent to handler |
| `Handler execution start` | INFO | dispatcher/core | Handler begins |
| `Handler execution complete` | INFO | dispatcher/core | Handler done |
| `Handler panicked - CRITICAL` | ERROR | dispatcher/core | Handler crashed |
| `Handler timeout or channel closed` | ERROR | dispatcher/core | Handler unresponsive |
| `Handler response received` | INFO | dispatcher/core | Response ready |
| `Spec change detected` | INFO | hot_reload | Hot reload triggered |
| `Spec reload success` | INFO | hot_reload | Reload completed |
| `Spec reload failed` | ERROR | hot_reload | Reload failed |

---

**Last Updated**: October 2025  
**Maintainer**: BRRTRouter Core Team  
**Feedback**: Submit issues to [GitHub](https://github.com/microscaler/BRRTRouter/issues)


