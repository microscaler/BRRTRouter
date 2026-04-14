# BRRTRouter Test Documentation

**Complete test suite documentation with coverage analysis**

## Test Organization

BRRTRouter has 31 test modules organized by functionality:

### 1. Core Component Tests

#### `server_tests.rs` ✅ Documented
**Coverage:** End-to-end HTTP server integration
- Server lifecycle management
- Request routing and dispatching  
- Authentication and authorization
- Keep-alive handling
- Echo handler functionality
**Strategy:** Full Pet Store API integration tests

#### `dispatcher_tests.rs` ✅ Documented  
**Coverage:** Request dispatcher and coroutine handlers
- Handler registration and lookup
- Request/response flow
- Typed handler conversion
- Middleware integration
- Panic recovery
**Strategy:** Unit + integration tests with mock handlers
**Known Issues:** Panic test ignored (May coroutine limitation)

#### `router_tests.rs` ✅ Documented
**Coverage:** Path matching and routing
- Path pattern → regex compilation
- Route matching by method/path
- Path parameter extraction
- Route priority (longest first)
**Strategy:** Synthetic OpenAPI specs with various patterns

### 2. Generator Tests

#### `generator_tests.rs` ✅ Documented
**Coverage:** Schema processing and type generation
- Case conversion (snake_case → CamelCase)
- Type mapping (JSON Schema → Rust)
- Field extraction with oneOf handling
- Example → Rust literal conversion
**Strategy:** Unit tests with synthetic schemas
**Goal:** 100% coverage of schema functions

#### `generator_templates_tests.rs`
**Coverage:** Askama template rendering
- Template compilation
- Variable substitution
- Import generation
- Output validation
**Strategy:** Template-specific unit tests

#### `generator_project_tests.rs`
**Coverage:** Full project generation
- File structure creation
- Cargo.toml generation
- Handler/controller generation
- Documentation generation
**Strategy:** End-to-end generation with cleanup

### 3. CLI Tests

#### `cli_tests.rs`
**Coverage:** Command-line interface
- `generate` command
- `serve` command  
- Argument parsing
- Error messages
**Strategy:** Subprocess execution tests

### 4. Spec Tests

#### `spec_tests.rs`
**Coverage:** OpenAPI specification loading
- YAML/JSON parsing
- Route metadata extraction
- Schema resolution
- Validation
**Strategy:** Various OpenAPI spec files
**Known Issues:** Flaky test with temp file race condition

#### `spec_helpers_tests.rs`
**Coverage:** Spec utility functions
- Reference resolution
- Schema expansion
- Type extraction
**Strategy:** Unit tests with spec fragments

### 5. Security & Auth Tests

#### `security_tests.rs`
**Coverage:** Authentication providers
- API key validation
- JWT validation (simple + JWKS)
- OAuth2 validation
- Remote API key provider
**Strategy:** Mock providers + integration tests

#### `auth_cors_tests.rs`
**Coverage:** Auth + CORS middleware
- CORS preflight handling
- Auth middleware integration
- Header validation
**Strategy:** Integration tests with live server

### 6. Middleware Tests

#### `middleware_tests.rs`
**Coverage:** Middleware system
- Middleware registration
- Execution order
- Request/response modification
- Short-circuit behavior
**Strategy:** Custom test middleware

#### `metrics_endpoint_tests.rs`
**Coverage:** Metrics collection
- Request counting
- Latency tracking
- Prometheus format
**Strategy:** Integration tests with `/metrics` endpoint

#### `tracing_tests.rs`
**Coverage:** Distributed tracing
- Span creation
- Context propagation
- OTLP export
**Strategy:** Test subscriber capture
**Known Issues:** Ignored (timing issues with May coroutines)

### 7. Feature Tests

#### `hot_reload_tests.rs`
**Coverage:** Live spec reloading
- File watching
- Route registration
- Debouncing
**Strategy:** Filesystem manipulation + timing

#### `sse_tests.rs` & `sse_channel_tests.rs`
**Coverage:** Server-Sent Events
- Channel creation
- Event streaming
- Connection lifecycle
**Strategy:** Integration tests with SSE endpoints

#### `static_files_tests.rs` & `static_server_tests.rs`
**Coverage:** Static file serving
- File serving
- MIME type detection
- Template rendering (MiniJinja)
- Path traversal prevention
**Strategy:** Test files + security tests

#### `typed_tests.rs`
**Coverage:** Type-safe handlers
- Type conversion
- Validation
- Error handling
**Strategy:** Typed handler examples

#### `param_style_tests.rs`
**Coverage:** OpenAPI parameter styles
- Form style
- Simple style
- Matrix style
- Array/object serialization
**Strategy:** Various parameter combinations

#### `multi_response_tests.rs`
**Coverage:** Multiple response types
- Different status codes
- Content negotiation
- Response selection
**Strategy:** Handlers with multiple responses

### 8. Endpoint Tests

#### `health_endpoint_tests.rs`
**Coverage:** `/health` endpoint
- Liveness check
- Response format
**Strategy:** Simple GET request test

#### `docs_endpoint_tests.rs`  
**Coverage:** `/docs` endpoint
- OpenAPI spec serving
- HTML documentation
**Strategy:** Endpoint availability tests

### 9. Integration Tests

#### `curl_integration_tests.rs`
**Coverage:** Docker-based integration
- Full Docker container tests
- Real HTTP client (curl)
- Multi-request scenarios
**Strategy:** Docker compose + curl harness
**Known Issues:** Slow (60+ seconds)

#### `docker_integration_tests.rs`
**Coverage:** Docker infrastructure
- Container health checks
- Network connectivity
**Strategy:** Docker API tests
**Known Issues:** Ignored (requires Docker)

#### `curl_harness.rs`
**Coverage:** Test infrastructure
- Docker container management
- Test isolation
**Strategy:** Shared test utility

### 10. Validation Tests

#### `validator_tests.rs`
**Coverage:** Request/response validation
- Parameter validation
- Schema validation
- Error reporting
**Strategy:** Valid + invalid requests

#### `dynamic_registration.rs`
**Coverage:** Runtime handler registration
- Dynamic route addition
- Live updates
**Strategy:** Programmatic registration

## Test Infrastructure

### Common Utilities

#### `tests/common/mod.rs`
**Purpose:** Shared test helpers
- HTTP client utilities
- Test server management
- Fixture data

#### `tracing_util.rs`
**Purpose:** Tracing test support
- Test subscriber
- Span capture
- Log filtering

## Test Execution

### Running Tests

```bash
# Standard cargo test
just test

# Fast parallel execution with nextest (recommended). Note that the first time this runs, it downloads some large docker containers 
# for build purposes and may show as a slow running test. This should improve on subsequent test runs.
just nt

# All 219 tests pass reliably with parallel execution ✅
```

### Run Specific Module
```bash
cargo test --test server_tests
```

### Run With Output
```bash
cargo test -- --nocapture
```

### Run Ignored Tests
```bash
cargo test -- --ignored
```

## Code Coverage

```bash
just coverage  # Generates HTML coverage report
# Must maintain ≥80% coverage
```

## Load Testing with Goose

BRRTRouter includes comprehensive load testing using [Goose](https://book.goose.rs/), which tests **ALL OpenAPI endpoints** (unlike wrk):

```bash
# Quick 30-second load test
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u10 -r2 -t30s \
  --header "X-API-Key: test123"

# Full load test with HTML report
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u20 -r5 -t2m \
  --no-reset-metrics \
  --header "X-API-Key: test123" \
  --report-file goose-report.html
```

**What Goose tests that wrk doesn't:**
- ✅ Authenticated endpoints (`GET /pets`, `/users` with API keys)
- ✅ All routes from OpenAPI spec (not just `/health`)
- ✅ Static files (`/openapi.yaml`, `/docs`, CSS, JS)
- ✅ Memory leak detection (sustained 2+ minute tests)
- ✅ Per-endpoint metrics with automatic failure detection

**CI Integration:**
Every PR runs a 2-minute Goose load test that tests 20 concurrent users across all endpoints and uploads ASCII metrics, HTML, and JSON reports.

See [docs/GOOSE_LOAD_TESTING.md](GOOSE_LOAD_TESTING.md) for complete guide.

## Running Benchmarks

```bash
just bench  # Executes cargo bench with Criterion
```

Recent profiling with `flamegraph` highlighted regex capture and `HashMap` allocations as hotspots. Preallocating buffers in `Router::route` and `path_to_regex` trimmed roughly 5% off benchmark times.

## Generating Flamegraphs

```bash
just flamegraph  # Produces flamegraph.svg in target/flamegraphs/
```

See [docs/flamegraph.md](flamegraph.md) for tips on reading the output.

## Coverage

### Current Coverage (Estimated)
- **Core Components:** ~85%
- **Generator:** ~90%
- **Security:** ~80%
- **Middleware:** ~75%
- **Features:** ~70%
- **Overall:** ~80%

### Coverage Gaps
1. Error paths in generated code
2. Edge cases in hot reload
3. Panic recovery in May coroutines
4. Docker integration (ignored tests)

## Known Issues & Flaky Tests

### High Priority
1. **`spec_tests.rs`**: Race condition in temp file generation
   - **Fix:** Use atomic counter + mutex for unique filenames

2. **`curl_integration_tests.rs`**: Timeout after 60 seconds
   - **Fix:** Image caching, parallel execution

### Medium Priority
3. **`tracing_tests.rs`**: Timing issues with async tracing
   - **Status:** Ignored
   - **Fix:** Better synchronization primitives

4. **`dispatcher_tests::test_panic_handler_returns_500`**: Panic test fails
   - **Status:** Ignored
   - **Issue:** May coroutines + catch_unwind incompatibility

### Low Priority
5. **`docker_integration_tests.rs`**: Requires Docker
   - **Status:** Ignored in CI
   - **Fix:** Optional test suite for Docker-enabled environments

## Test Quality Metrics

- **Total Tests:** 150+
- **Passing:** 145+
- **Ignored:** 5
- **Average Runtime:** ~30 seconds
- **Slowest:** curl_integration_tests (60s)
- **Fastest:** Unit tests (<1ms)

## Contributing

When adding new tests:
1. Add module-level documentation (`//!`)
2. Document test strategy and coverage
3. Note any known issues or limitations
4. Ensure tests are deterministic
5. Avoid global state
6. Clean up resources

## Future Improvements

- [ ] Reduce test runtime with parallelization
- [ ] Fix flaky tests (spec_tests race condition)
- [ ] Increase coverage to 90%+
- [ ] Add property-based tests for generator
- [ ] Add fuzzing for request parsing
- [ ] Add benchmark tests

