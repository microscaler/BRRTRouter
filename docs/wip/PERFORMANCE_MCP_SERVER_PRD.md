# Product Requirements Document: Performance Testing MCP Server

**Version:** 1.1  
**Date:** December 2025  
**Status:** Draft  
**Owner:** BRRTRouter Development Team

**Changelog:**
- v1.1: Added CI checks integration (tests, linting, coverage, formatting, documentation)
- v1.0: Initial PRD with performance testing focus

---

## Executive Summary

This PRD defines requirements for a Model Context Protocol (MCP) server that provides automated performance testing capabilities for BRRTRouter. The server will enable AI agents to run comprehensive performance tests, compare results against baselines, and detect regressions automatically as part of the development workflow.

**Problem Statement:** Currently, performance testing requires manual orchestration of multiple commands (`just debug-petstore`, `just goose-jsf`, manual comparison). Additionally, CI failures (tests, linting, coverage) are discovered only after pushing to GitHub, creating wasted cycles and blocking PRs. This creates friction in the development workflow, especially during JSF (JavaScript Foundation) optimization work where performance must be validated after each change.

**Solution:** An MCP server that wraps existing performance testing infrastructure and CI checks, providing a standardized interface for AI agents to execute full performance test workflows and pre-commit validation automatically. This enables catching CI failures locally before pushing to GitHub.

---

## Goals and Objectives

### Primary Goals

1. **Automate Performance Testing Workflow**
   - Enable AI agents to run complete performance test suites with a single command
   - Eliminate manual orchestration of service lifecycle and test execution
   - Provide consistent test configurations across all runs

2. **Automated Regression Detection**
   - Automatically compare test results against established baselines
   - Detect performance regressions with configurable thresholds
   - Provide actionable reports for performance analysis

3. **Integration with Existing Infrastructure**
   - Wrap existing `scripts/run_goose_tests.py` and `justfile` commands
   - Maintain compatibility with current manual workflows
   - Follow CLI-first development principles (no shell script bypasses)

4. **Historical Performance Tracking**
   - Store test results with metadata (branch, commit, timestamp)
   - Enable trend analysis over time
   - Support multiple baseline management

5. **Pre-Commit CI Validation**
   - Run all CI checks locally before pushing to GitHub
   - Catch test failures, linting errors, and coverage issues early
   - Provide actionable feedback matching CI output format
   - Reduce CI failure rate and PR iteration cycles

### Success Criteria

- ✅ AI agents can run full performance test workflow (start → test → compare → report) with a single MCP call
- ✅ Automated regression detection with <5% false positive rate
- ✅ Test execution time overhead <10% compared to direct script execution
- ✅ 100% backward compatibility with existing `justfile` and Python scripts
- ✅ Support for at least 3 concurrent test runs (different configurations)
- ✅ Baseline comparison accuracy matches existing `compare_metrics.py` output
- ✅ CI checks can be run locally with identical results to GitHub Actions
- ✅ >90% reduction in CI failures due to pre-commit validation
- ✅ All CI checks (tests, linting, coverage, docs) executable via MCP

---

## Requirements

### Functional Requirements

#### FR1: Service Lifecycle Management

**FR1.1: Start Petstore Service**
- **Description:** Start the petstore service with configurable options
- **Inputs:**
  - `config` (optional): Service configuration (stack size, log level, port, etc.)
  - `build` (optional): Whether to rebuild before starting (default: false)
- **Outputs:**
  - `status`: Service status (running, starting, error)
  - `pid`: Process ID if running
  - `port`: Listening port
  - `health_url`: Health check endpoint URL
- **Error Handling:**
  - Detect if service is already running
  - Handle port conflicts gracefully
  - Provide clear error messages for build failures

**FR1.2: Stop Petstore Service**
- **Description:** Gracefully stop the petstore service
- **Inputs:** None (stops default service)
- **Outputs:**
  - `success`: Boolean indicating if stop was successful
  - `was_running`: Boolean indicating if service was running before stop
- **Error Handling:**
  - Handle cases where service is not running
  - Force kill if graceful shutdown fails after timeout

**FR1.3: Restart Petstore Service**
- **Description:** Stop and start service in one operation
- **Inputs:** Same as FR1.1
- **Outputs:** Same as FR1.1

**FR1.4: Get Service Status**
- **Description:** Check current status of petstore service
- **Inputs:** None
- **Outputs:**
  - `status`: running, stopped, starting, error
  - `uptime`: Seconds since start (if running)
  - `health_check`: Result of health endpoint check
  - `metrics_endpoint`: Prometheus metrics endpoint status

#### FR2: Performance Test Execution

**FR2.1: Run Standard Performance Test**
- **Description:** Execute standard Goose load test with configurable parameters
- **Inputs:**
  - `label`: Unique identifier for this test run (required)
  - `users`: Number of concurrent users (default: 2000)
  - `run_time`: Test duration (default: "60s")
  - `hatch_rate`: Users per second to start (default: 200)
  - `warmup_time`: Warmup duration before test (default: 10s)
  - `runs`: Number of test runs to average (default: 3)
- **Outputs:**
  - `test_id`: Unique test run identifier
  - `results`: Aggregated metrics (throughput, latency, failures)
  - `individual_runs`: Metrics for each individual run
  - `output_files`: Paths to generated output files
  - `duration`: Total test execution time

**FR2.2: Run Quick Smoke Test**
- **Description:** Execute brief performance test for quick validation
- **Inputs:**
  - `label`: Test label (optional, auto-generated if not provided)
- **Outputs:**
  - Same as FR2.1, but with reduced test parameters (5 users, 10s runtime)
- **Use Case:** Pre-commit validation, quick regression checks

**FR2.3: Run Full JSF Suite**
- **Description:** Execute comprehensive JSF performance test suite
- **Inputs:**
  - `label`: Test label (required)
  - `baseline_label`: Baseline to compare against (optional)
- **Outputs:**
  - Same as FR2.1
  - `comparison`: Comparison report if baseline provided
- **Use Case:** Post-optimization validation, release candidate testing

**FR2.4: Run Custom Test Configuration**
- **Description:** Execute test with fully custom configuration
- **Inputs:**
  - All parameters from FR2.1
  - `custom_config`: Additional custom parameters
- **Outputs:** Same as FR2.1

#### FR3: Baseline Management

**FR3.1: Set Baseline**
- **Description:** Mark a test result as a baseline for future comparisons
- **Inputs:**
  - `test_id`: Test run identifier to use as baseline
  - `label`: Baseline label (required)
  - `description`: Optional description
- **Outputs:**
  - `baseline_id`: Unique baseline identifier
  - `baseline_metrics`: Metrics stored as baseline
- **Error Handling:**
  - Validate test_id exists
  - Prevent overwriting existing baselines without explicit flag

**FR3.2: List Baselines**
- **Description:** List all available baselines
- **Inputs:**
  - `filter`: Optional filter by label pattern
- **Outputs:**
  - `baselines`: Array of baseline objects with metadata
  - Each baseline includes: label, test_id, timestamp, metrics summary

**FR3.3: Get Baseline**
- **Description:** Retrieve baseline details and metrics
- **Inputs:**
  - `label`: Baseline label (required)
- **Outputs:**
  - `baseline`: Complete baseline object with all metrics
  - `metadata`: Creation timestamp, associated test_id, description

**FR3.4: Delete Baseline**
- **Description:** Remove a baseline
- **Inputs:**
  - `label`: Baseline label (required)
- **Outputs:**
  - `success`: Boolean indicating deletion success
- **Error Handling:**
  - Handle non-existent baselines gracefully

#### FR4: Comparison and Analysis

**FR4.1: Compare with Baseline**
- **Description:** Compare test results against a baseline
- **Inputs:**
  - `test_id`: Test run identifier (required)
  - `baseline_label`: Baseline label (required)
  - `thresholds`: Optional custom regression thresholds
- **Outputs:**
  - `comparison`: Detailed comparison report
  - `regressions`: Array of detected regressions
  - `improvements`: Array of detected improvements
  - `summary`: High-level summary (pass/fail, overall change %)
- **Comparison Metrics:**
  - Throughput (req/s)
  - Latency percentiles (P50, P75, P98, P99)
  - Failure rate
  - Error distribution

**FR4.2: Detect Regressions**
- **Description:** Automatically detect performance regressions
- **Inputs:**
  - `test_id`: Test run identifier (required)
  - `baseline_label`: Baseline label (required)
  - `thresholds`: Regression thresholds (default: 5% degradation)
- **Outputs:**
  - `has_regression`: Boolean indicating if regressions detected
  - `regressions`: Array of regression details
  - `severity`: Overall severity (none, minor, major, critical)
  - `recommendation`: Suggested action (investigate, rollback, etc.)

**FR4.3: Get Performance Trends**
- **Description:** Analyze performance trends over time
- **Inputs:**
  - `timeframe`: Time range to analyze (default: last 30 days)
  - `metric`: Specific metric to trend (optional, default: throughput)
- **Outputs:**
  - `trends`: Array of data points over time
  - `direction`: Overall trend direction (improving, stable, degrading)
  - `volatility`: Measure of result variance

#### FR5: Resource Management

**FR5.1: List Test Results**
- **Description:** List all available test results
- **Inputs:**
  - `filter`: Optional filter by label pattern, date range, or branch
  - `limit`: Maximum number of results to return (default: 50)
- **Outputs:**
  - `results`: Array of test result summaries
  - Each result includes: test_id, label, timestamp, key metrics, status

**FR5.2: Get Test Result**
- **Description:** Retrieve detailed test result
- **Inputs:**
  - `test_id`: Test run identifier (required)
- **Outputs:**
  - `result`: Complete test result with all metrics
  - `metadata`: Test configuration, system info, execution details
  - `files`: Associated output files (JSON, HTML reports)

**FR5.3: Export Test Result**
- **Description:** Export test result in various formats
- **Inputs:**
  - `test_id`: Test run identifier (required)
  - `format`: Export format (json, markdown, html, csv)
- **Outputs:**
  - `export_data`: Formatted test result data
  - `export_file`: Path to exported file (if file-based export)

#### FR6: CI Checks and Validation

**FR6.1: Run All CI Checks**
- **Description:** Execute all CI checks locally (matching GitHub Actions workflow)
- **Inputs:**
  - `checks`: Array of check names to run (optional, default: all)
  - `fail_fast`: Stop on first failure (default: true)
  - `coverage`: Include coverage measurement (default: true)
- **Outputs:**
  - `results`: Array of check results
  - `overall_status`: pass, fail, or partial
  - `summary`: High-level summary of all checks
  - `duration`: Total execution time
- **CI Checks Included:**
  - Build check (`cargo build`)
  - Code generation check
  - Linting (`cargo clippy`)
  - Documentation check (`cargo doc`)
  - Test execution with coverage (`cargo llvm-cov nextest`)
  - Format check (`cargo fmt --check`)

**FR6.2: Run Tests with Coverage**
- **Description:** Execute tests using nextest with coverage measurement (matches CI)
- **Inputs:**
  - `targets`: Specific targets to test (optional, default: all)
  - `fail_fast`: Stop on first failure (default: true)
  - `output_format`: Coverage output format (html, json, lcov, default: html)
- **Outputs:**
  - `test_results`: Test execution results
  - `coverage_report`: Coverage metrics and report location
  - `junit_report`: JUnit XML report path (if generated)
  - `passed`: Number of passing tests
  - `failed`: Number of failing tests
  - `coverage_percentage`: Overall code coverage percentage

**FR6.3: Run Linting Checks**
- **Description:** Execute Clippy linting checks (matches CI)
- **Inputs:**
  - `deny_warnings`: Treat warnings as errors (default: true, matches CI)
  - `all_targets`: Run on all targets (default: false, matches CI)
  - `all_features`: Run with all features (default: false, matches CI)
- **Outputs:**
  - `status`: pass or fail
  - `warnings`: Array of warning messages
  - `errors`: Array of error messages
  - `suggestions`: Array of suggested fixes
  - `summary`: Count of warnings/errors by category

**FR6.4: Check Code Formatting**
- **Description:** Verify code formatting matches `cargo fmt` standards
- **Inputs:**
  - `check_only`: Only check, don't format (default: true)
  - `format_on_fail`: Auto-format if check fails (default: false)
- **Outputs:**
  - `status`: pass or fail
  - `formatted_files`: Array of files that need formatting (if check_only)
  - `formatted_count`: Number of files formatted (if format_on_fail)

**FR6.5: Check Documentation**
- **Description:** Verify documentation builds and has no broken links (matches CI)
- **Inputs:**
  - `deny_warnings`: Treat warnings as errors (default: true, matches CI)
  - `check_links`: Check for broken intra-doc links (default: true)
- **Outputs:**
  - `status`: pass or fail
  - `warnings`: Array of documentation warnings
  - `broken_links`: Array of broken link errors
  - `doc_path`: Path to generated documentation

**FR6.6: Run Build Check**
- **Description:** Verify code compiles successfully (matches CI)
- **Inputs:**
  - `release`: Build in release mode (default: false)
  - `all_features`: Build with all features (default: false)
  - `targets`: Specific targets to build (optional)
- **Outputs:**
  - `status`: pass or fail
  - `build_artifacts`: Array of built artifact paths
  - `warnings`: Array of build warnings
  - `errors`: Array of build errors

**FR6.7: Run Code Generation Check**
- **Description:** Verify code generation works correctly (matches CI)
- **Inputs:**
  - `spec_file`: OpenAPI spec file (default: examples/openapi.yaml)
  - `force`: Force regeneration (default: true, matches CI)
- **Outputs:**
  - `status`: pass or fail
  - `generated_files`: Array of generated file paths
  - `errors`: Array of generation errors

**FR6.8: Get CI Check Status**
- **Description:** Get status of last CI check run
- **Inputs:**
  - `check_name`: Specific check name (optional)
- **Outputs:**
  - `last_run`: Timestamp of last run
  - `status`: Overall status
  - `checks`: Array of individual check statuses
  - `summary`: Summary of results

**FR6.9: Compare Local vs CI Results**
- **Description:** Compare local CI check results with GitHub Actions results
- **Inputs:**
  - `pr_number`: PR number to compare against (optional)
  - `commit_sha`: Commit SHA to compare against (optional)
- **Outputs:**
  - `comparison`: Detailed comparison of local vs CI results
  - `differences`: Array of differences found
  - `recommendations`: Suggested actions based on differences

### Non-Functional Requirements

#### NFR1: Performance

- **NFR1.1:** Test execution overhead <10% compared to direct script execution
- **NFR1.2:** Service start/stop operations complete within 30 seconds
- **NFR1.3:** Baseline comparison completes within 2 seconds for standard test results
- **NFR1.4:** Support concurrent execution of up to 3 different test configurations

#### NFR2: Reliability

- **NFR2.1:** 99.9% uptime for MCP server (excluding planned maintenance)
- **NFR2.2:** Graceful handling of service crashes during tests
- **NFR2.3:** Automatic cleanup of orphaned processes
- **NFR2.4:** Data persistence: test results survive server restarts

#### NFR3: Compatibility

- **NFR3.1:** 100% backward compatibility with existing `justfile` commands
- **NFR3.2:** 100% compatibility with existing `scripts/run_goose_tests.py` output format
- **NFR3.3:** Support for all existing test configurations
- **NFR3.4:** No breaking changes to existing baseline storage format
- **NFR3.5:** CI check results match GitHub Actions output exactly
- **NFR3.6:** Same command-line arguments and flags as CI workflow

#### NFR4: Usability

- **NFR4.1:** Clear, actionable error messages for all failure modes
- **NFR4.2:** Comprehensive logging for debugging
- **NFR4.3:** Structured output suitable for programmatic consumption
- **NFR4.4:** Documentation for all MCP tools and resources

#### NFR5: Security

- **NFR5.1:** No execution of arbitrary commands (only whitelisted operations)
- **NFR5.2:** Input validation for all parameters
- **NFR5.3:** Safe handling of file paths (prevent directory traversal)
- **NFR5.4:** Process isolation for test execution

---

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────────┐
│                    MCP Server (Python)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │ Service      │  │ Test        │  │ Baseline     │    │
│  │ Manager      │  │ Executor    │  │ Manager      │    │
│  └──────┬───────┘  └──────┬──────┘  └──────┬───────┘    │
│  ┌──────┴───────┐  ┌──────┴──────┐         │            │
│  │ CI Check     │  │ Comparison  │         │            │
│  │ Runner       │  │ Engine      │         │            │
│  └──────┬───────┘  └──────┬──────┘         │            │
│         │                 │                 │            │
└─────────┼─────────────────┼─────────────────┼────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│              Existing Infrastructure (CLI)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │ justfile     │  │ run_goose_  │  │ compare_     │    │
│  │ commands     │  │ tests.py    │  │ metrics.py   │    │
│  └──────┬───────┘  └──────┬──────┘  └──────┬───────┘    │
│  ┌──────┴───────┐  ┌──────┴──────┐  ┌──────┴───────┐    │
│  │ cargo        │  │ cargo       │  │ cargo       │    │
│  │ (build/test) │  │ (clippy)    │  │ (fmt/doc)   │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
└─────────────────────────────────────────────────────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│                    Test Infrastructure                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │ pet_store    │  │ Goose        │  │ Metrics      │    │
│  │ Service      │  │ Load Test    │  │ Storage      │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### MCP Server Structure

```
tools/performance_mcp/
├── __init__.py
├── server.py                 # Main MCP server implementation
├── service_manager.py        # Service lifecycle management
├── test_executor.py          # Test execution wrapper
├── baseline_manager.py       # Baseline CRUD operations
├── comparison_engine.py      # Comparison and regression detection
├── ci_check_runner.py        # CI checks execution (tests, linting, etc.)
├── storage.py                # Test result storage and retrieval
├── config.py                 # Configuration management
└── utils.py                  # Utility functions

tests/
└── test_performance_mcp.py   # Unit and integration tests

docs/
└── PERFORMANCE_MCP_USAGE.md  # Usage documentation
```

### Data Storage

**Test Results Storage:**
- Location: `performance_results/` directory (configurable)
- Format: JSON files with naming: `{label}_metrics.json`, `{label}_run{N}_metrics.json`
- Metadata: Stored alongside metrics (timestamp, git commit, branch, config)

**Baseline Storage:**
- Location: `performance_results/baselines/` directory
- Format: JSON files with naming: `{baseline_label}_baseline.json`
- Content: Reference to test result + baseline metadata

**MCP Server State:**
- Location: `performance_results/.mcp_state/` directory
- Content: Service PIDs, active test runs, locks

### MCP Tools (Actions)

**Service Management:**
1. **start_petstore** - Start petstore service
2. **stop_petstore** - Stop petstore service
3. **restart_petstore** - Restart petstore service
4. **get_service_status** - Get service status

**Performance Testing:**
5. **run_performance_test** - Execute standard performance test
6. **run_smoke_test** - Execute quick smoke test
7. **run_jsf_suite** - Execute full JSF test suite

**Baseline Management:**
8. **set_baseline** - Mark test result as baseline
9. **list_baselines** - List all baselines
10. **get_baseline** - Get baseline details
11. **delete_baseline** - Delete baseline

**Comparison and Analysis:**
12. **compare_with_baseline** - Compare test with baseline
13. **detect_regressions** - Detect performance regressions
14. **get_performance_trends** - Analyze trends over time

**Test Results:**
15. **list_test_results** - List test results
16. **get_test_result** - Get test result details
17. **export_test_result** - Export test result

**CI Checks (NEW):**
18. **run_all_ci_checks** - Run all CI checks locally
19. **run_tests_with_coverage** - Run tests with coverage (nextest + llvm-cov)
20. **run_linting_checks** - Run Clippy linting
21. **check_code_formatting** - Check code formatting (cargo fmt)
22. **check_documentation** - Check documentation (cargo doc)
23. **run_build_check** - Verify code compiles
24. **run_code_generation_check** - Verify code generation works
25. **get_ci_check_status** - Get status of last CI check run
26. **compare_local_vs_ci** - Compare local vs GitHub Actions results

### MCP Resources

1. **performance-results://{test_id}** - Test result data
2. **performance-results://baselines/{label}** - Baseline data
3. **performance-results://comparisons/{test_id}_vs_{baseline}** - Comparison reports
4. **performance-results://trends/{metric}** - Performance trends

---

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1)

**Deliverables:**
- MCP server skeleton with basic tool registration
- Service manager (start/stop/status)
- Integration with `justfile` commands
- Basic error handling and logging

**Acceptance Criteria:**
- Can start/stop petstore service via MCP
- Service status accurately reported
- Error handling for common failure modes

### Phase 2: Test Execution (Week 1-2)

**Deliverables:**
- Test executor wrapper for `run_goose_tests.py`
- Support for standard, smoke, and JSF test configurations
- Test result storage and retrieval
- Integration with existing output formats

**Acceptance Criteria:**
- Can execute all test types via MCP
- Test results stored in compatible format
- Output matches existing script output

### Phase 3: Baseline Management (Week 2)

**Deliverables:**
- Baseline CRUD operations
- Baseline storage and retrieval
- Integration with existing baseline format

**Acceptance Criteria:**
- Can create, list, get, and delete baselines
- Baselines compatible with existing comparison tools
- Baseline metadata properly stored

### Phase 4: Comparison and Analysis (Week 2-3)

**Deliverables:**
- Comparison engine wrapping `compare_metrics.py`
- Regression detection with configurable thresholds
- Trend analysis functionality
- Comparison report generation

**Acceptance Criteria:**
- Comparison results match existing tool output
- Regression detection with <5% false positive rate
- Trend analysis provides actionable insights

### Phase 5: CI Checks Integration (Week 3)

**Deliverables:**
- CI check runner wrapping cargo commands
- Support for all CI checks (build, test, lint, format, doc)
- Coverage measurement integration (cargo-llvm-cov)
- Test execution with nextest
- CI result comparison with GitHub Actions

**Acceptance Criteria:**
- All CI checks executable via MCP
- Results match GitHub Actions output exactly
- Coverage reports generated correctly
- Can compare local vs CI results
- Pre-commit validation workflow functional

### Phase 6: Resource Management and Export (Week 3-4)

**Deliverables:**
- Test result listing and filtering
- Test result export in multiple formats
- Resource endpoints for MCP clients
- Comprehensive error handling

**Acceptance Criteria:**
- Can list and filter test results
- Export works for all supported formats
- Resources accessible via MCP protocol

### Phase 7: Testing and Documentation (Week 4)

**Deliverables:**
- Comprehensive unit tests (>80% coverage)
- Integration tests with real service
- Usage documentation
- Example workflows

**Acceptance Criteria:**
- All tests passing
- Documentation complete and accurate
- Example workflows validated

---

## Technical Specifications

### Technology Stack

- **Language:** Python 3.9+
- **MCP Framework:** `mcp` Python SDK (latest stable)
- **Dependencies:**
  - `subprocess` - Execute CLI commands
  - `pathlib` - File system operations
  - `json` - Data serialization
  - `datetime` - Timestamp handling
  - `psutil` - Process management (optional, for enhanced status)

### Integration Points

**Justfile Commands:**
- `just debug-petstore` - Service start
- `just stop-petstore` - Service stop
- `just goose-jsf` - Performance test execution

**Python Scripts:**
- `scripts/run_goose_tests.py` - Test execution
- `scripts/compare_metrics.py` - Metrics comparison

**File System:**
- Test results: `performance_results/` (or configurable)
- Baselines: `performance_results/baselines/`
- State: `performance_results/.mcp_state/`

### Configuration

**MCP Server Configuration:**
```json
{
  "results_directory": "performance_results",
  "default_test_config": {
    "users": 2000,
    "run_time": "60s",
    "hatch_rate": 200,
    "warmup_time": 10,
    "runs": 3
  },
  "regression_thresholds": {
    "throughput_degradation": 0.05,
    "latency_increase": 0.10,
    "failure_rate_increase": 0.01
  },
  "service_config": {
    "port": 8080,
    "health_check_timeout": 30,
    "graceful_shutdown_timeout": 10
  }
}
```

### Error Handling

**Error Categories:**
1. **Service Errors:** Service start/stop failures, health check failures
2. **Test Errors:** Test execution failures, timeout errors
3. **Storage Errors:** File I/O errors, permission errors
4. **Validation Errors:** Invalid parameters, missing dependencies
5. **Comparison Errors:** Missing baselines, incompatible formats

**Error Response Format:**
```json
{
  "error": {
    "code": "SERVICE_START_FAILED",
    "message": "Failed to start petstore service",
    "details": "...",
    "recoverable": true,
    "suggested_action": "Check if port 8080 is already in use"
  }
}
```

---

## Testing Strategy

### Unit Tests

- Service manager operations (mock subprocess)
- Test executor parameter validation
- Baseline manager CRUD operations
- Comparison engine calculations
- Storage operations

### Integration Tests

- Full workflow: start → test → compare → stop
- Baseline creation and comparison
- Regression detection accuracy
- Concurrent test execution
- Error recovery scenarios

### Performance Tests

- Overhead measurement vs direct script execution
- Concurrent test execution limits
- Large baseline comparison performance
- Storage I/O performance

---

## Risks and Mitigations

### Risk 1: Process Management Complexity

**Risk:** Managing service lifecycle across different platforms (macOS, Linux) may be complex.

**Mitigation:**
- Use existing `justfile` commands which already handle platform differences
- Implement robust process detection using `psutil` or `pgrep`
- Comprehensive error handling and logging

### Risk 2: Test Result Format Compatibility

**Risk:** Changes to existing script output format could break MCP server.

**Mitigation:**
- Version test result format
- Implement format detection and migration
- Maintain backward compatibility layer

### Risk 3: Concurrent Test Execution Conflicts

**Risk:** Multiple concurrent tests may conflict (port usage, resource contention).

**Mitigation:**
- Implement test execution locks
- Support configurable ports for concurrent runs
- Resource usage monitoring

### Risk 4: False Positive Regressions

**Risk:** Regression detection may flag false positives due to test variance.

**Mitigation:**
- Configurable thresholds
- Statistical analysis (confidence intervals)
- Multiple run averaging (already in existing script)
- Manual override capability

### Risk 5: Storage Growth

**Risk:** Test results may accumulate and consume disk space.

**Mitigation:**
- Implement retention policies
- Compression for old results
- Configurable cleanup schedules

### Risk 6: CI Check Environment Differences

**Risk:** Local CI check results may differ from GitHub Actions due to environment differences (OS, Rust version, dependencies).

**Mitigation:**
- Use same Rust toolchain version as CI
- Document environment requirements
- Provide environment validation check
- Compare local vs CI results tool to identify differences
- Use Docker/containerization for consistent environments (future enhancement)

---

## Success Metrics

### Quantitative Metrics

- **Test Execution Time:** <10% overhead vs direct script execution
- **Regression Detection Accuracy:** >95% true positive rate, <5% false positive rate
- **Service Start Time:** <30 seconds from command to ready
- **Comparison Speed:** <2 seconds for standard test results
- **Test Coverage:** >80% code coverage
- **CI Check Execution Time:** <5 minutes for full CI suite (matches GitHub Actions)
- **CI Failure Reduction:** >90% reduction in GitHub Actions failures due to pre-commit validation
- **CI Check Accuracy:** 100% match between local and GitHub Actions results (when environments match)

### Qualitative Metrics

- **Developer Experience:** AI agents can run full workflow without manual intervention
- **Reliability:** No data loss, consistent results across runs
- **Usability:** Clear error messages, actionable recommendations
- **Maintainability:** Well-documented, testable code structure
- **CI Integration:** Seamless pre-commit validation prevents GitHub failures
- **Feedback Quality:** CI check failures provide actionable, specific guidance matching CI output

---

## Future Enhancements

### Phase 2 Features (Post-MVP)

1. **Distributed Testing:** Support for testing across multiple machines
2. **Real-time Monitoring:** Live metrics during test execution
3. **Automated Optimization Suggestions:** AI-powered performance recommendations
4. **Advanced CI Integration:** Direct GitHub Actions API integration, PR comment automation
5. **Performance Budgets:** Enforce performance budgets per endpoint
6. **Historical Analysis:** Long-term trend visualization
7. **Custom Metrics:** Support for custom metric collection
8. **Multi-baseline Comparison:** Compare against multiple baselines simultaneously
9. **Docker-based CI Checks:** Containerized CI checks for 100% environment parity
10. **Git Hooks Integration:** Automatic pre-commit CI validation

---

## Dependencies

### External Dependencies

- Python 3.9+
- MCP Python SDK
- Existing BRRTRouter test infrastructure
- `just` command-line tool
- `cargo` (Rust toolchain)
- `cargo-nextest` (for test execution)
- `cargo-llvm-cov` (for coverage measurement)
- `cargo-clippy` (for linting, part of Rust toolchain)
- Rust stable toolchain (matching CI version)

### Internal Dependencies

- `scripts/run_goose_tests.py` (must remain compatible)
- `scripts/compare_metrics.py` (must remain compatible)
- `justfile` commands (must remain compatible)
- Test result storage format (must remain compatible)

---

## Open Questions

1. **Storage Location:** Should test results be stored in git (tracked) or gitignored?
   - **Recommendation:** Gitignored by default, with option to commit specific results

2. **Baseline Naming:** Should baselines be branch-specific or global?
   - **Recommendation:** Support both, with branch-specific as default

3. **Concurrent Test Limits:** Maximum number of concurrent tests?
   - **Recommendation:** Configurable, default to 3

4. **Result Retention:** How long should test results be retained?
   - **Recommendation:** 90 days default, configurable retention policy

5. **MCP Server Deployment:** Standalone server or integrated into existing tooling?
   - **Recommendation:** Standalone server for maximum flexibility

---

## Approval

**Product Owner:** [TBD]  
**Technical Lead:** [TBD]  
**Engineering Manager:** [TBD]

**Status:** Ready for Implementation

---

## Appendix

### A. Example MCP Tool Calls

```python
# Start service
{
  "tool": "start_petstore",
  "arguments": {
    "config": {
      "port": 8080,
      "log_level": "info"
    },
    "build": false
  }
}

# Run performance test
{
  "tool": "run_performance_test",
  "arguments": {
    "label": "jsf-p1-optimization",
    "users": 2000,
    "run_time": "60s",
    "baseline_label": "jsf-p0-2"
  }
}

# Detect regressions
{
  "tool": "detect_regressions",
  "arguments": {
    "test_id": "jsf-p1-optimization",
    "baseline_label": "jsf-p0-2",
    "thresholds": {
      "throughput_degradation": 0.05,
      "latency_increase": 0.10
    }
  }
}

# Run all CI checks (pre-commit validation)
{
  "tool": "run_all_ci_checks",
  "arguments": {
    "fail_fast": true,
    "coverage": true
  }
}

# Run tests with coverage (matches CI)
{
  "tool": "run_tests_with_coverage",
  "arguments": {
    "fail_fast": true,
    "output_format": "html"
  }
}

# Run linting checks
{
  "tool": "run_linting_checks",
  "arguments": {
    "deny_warnings": true
  }
}
```

### B. Example Workflows

**Performance Testing Workflow:**
```python
# AI Agent Performance Testing Workflow
1. start_petstore() → Wait for health check
2. run_performance_test(label="feature-x", baseline="main") → Get test_id
3. detect_regressions(test_id, baseline="main") → Check for regressions
4. if regressions:
     - get_test_result(test_id) → Analyze details
     - compare_with_baseline(test_id, baseline="main") → Get full comparison
     - Alert developer or rollback
5. stop_petstore() → Cleanup
```

**Pre-Commit CI Validation Workflow:**
```python
# AI Agent Pre-Commit Workflow
1. run_all_ci_checks(fail_fast=true) → Run all CI checks
2. if any check fails:
     - get_ci_check_status() → Get detailed failure information
     - if linting fails:
         - run_linting_checks() → Get specific lint errors
         - Fix linting issues
     - if tests fail:
         - run_tests_with_coverage() → Get test failure details
         - Fix test failures
     - if formatting fails:
         - check_code_formatting(format_on_fail=true) → Auto-format
     - Re-run checks until all pass
3. All checks pass → Safe to commit/push
```

**Full Development Cycle Workflow:**
```python
# Complete development cycle with CI + Performance
1. run_all_ci_checks() → Ensure code quality
2. if CI checks pass:
     - start_petstore()
     - run_performance_test(label="feature-x")
     - detect_regressions(test_id, baseline="main")
     - if no regressions:
         - stop_petstore()
         - Ready for commit
     - else:
         - Investigate and fix performance issues
         - Re-run performance test
3. Commit and push (CI will pass)
```

### C. File Structure Example

```
performance_results/
├── jsf-p0-1_metrics.json
├── jsf-p0-1_run1_metrics.json
├── jsf-p0-1_run2_metrics.json
├── jsf-p0-1_run3_metrics.json
├── jsf-p0-2_metrics.json
├── jsf-p0-2_run1_metrics.json
├── jsf-p0-2_run2_metrics.json
├── jsf-p0-2_run3_metrics.json
├── jsf-p0-2_vs_jsf-p0-1.json
├── baselines/
│   ├── jsf-p0-1_baseline.json
│   └── jsf-p0-2_baseline.json
└── .mcp_state/
    ├── service.pid
    └── active_tests.json
```

---

**Document Version History:**
- v1.1 (2025-12-XX): Added CI checks integration (tests, linting, coverage, formatting, documentation)
- v1.0 (2025-12-XX): Initial PRD draft

