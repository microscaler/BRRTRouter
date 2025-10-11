# Nextest CI Migration

## Change Summary

Added `cargo nextest` as the primary test runner in CI, with `cargo test` as a fallback.

## Why Nextest?

### Benefits Over `cargo test`

1. **Better Isolation**
   - Each test runs in its own process
   - Prevents state leakage between tests
   - More reliable test results

2. **Faster Execution**
   - Smarter parallelization
   - Better CPU utilization
   - Reduced overall test time

3. **Better Output**
   - Cleaner test results
   - Clear failure reporting
   - Progress indicators

4. **Flaky Test Handling**
   - Built-in retry logic
   - Better at handling timing issues
   - More robust in CI environments

5. **Already Used Locally**
   - `just nt` uses nextest
   - Developers already familiar
   - Consistent local/CI experience

## Implementation

### CI Workflow Changes

**File:** `.github/workflows/ci.yml`

```yaml
# Install nextest
- name: Install cargo-nextest
  run: cargo install cargo-nextest --locked

# Primary test runner
- name: Run tests (nextest - primary)
  run: cargo nextest run --workspace --all-targets --fail-fast --no-capture
  env:
    RUST_BACKTRACE: 1

# Fallback (only if nextest fails)
- name: Run tests (cargo test - legacy fallback)
  if: failure()
  run: cargo test -- --nocapture
```

### Key Features

#### 1. Primary Runner
```yaml
run: cargo nextest run --workspace --all-targets --fail-fast --no-capture
```
- `--workspace`: Run all tests in the workspace
- `--all-targets`: Include integration tests, doc tests, etc.
- `--fail-fast`: Stop on first failure (fast feedback)
- `--no-capture`: Show test output (like `--nocapture` in cargo test)

#### 2. Legacy Fallback
```yaml
if: failure()
run: cargo test -- --nocapture
```
- Only runs if nextest fails
- Provides backup in case nextest has issues
- Can be removed once we confirm stability

## Migration Strategy

### Phase 1: Dual Runner (Current)
- ✅ Nextest runs first (primary)
- ✅ Cargo test runs if nextest fails (backup)
- ✅ Monitor nextest stability

### Phase 2: Nextest Only (Future)
After confirming nextest works reliably:
- Remove the `cargo test` fallback step
- Keep nextest as sole test runner
- Update documentation

### Phase 3: Optimization (Later)
Once stable, optimize nextest configuration:
- Add retries for flaky tests
- Customize output format
- Tune parallelization

## Expected Behavior

### Successful Run (Ideal Path)
```
✓ Install cargo-nextest
✓ Run tests (nextest - primary) - ALL PASS
⊘ Run tests (cargo test - legacy fallback) - SKIPPED
```
**Result:** Fast, clean test execution

### Nextest Failure (Fallback Path)
```
✓ Install cargo-nextest
✗ Run tests (nextest - primary) - FAILED
✓ Run tests (cargo test - legacy fallback) - PASS
```
**Result:** We get diagnostic info from both runners

### Both Fail (Real Failure)
```
✓ Install cargo-nextest
✗ Run tests (nextest - primary) - FAILED
✗ Run tests (cargo test - legacy fallback) - FAILED
```
**Result:** Clear indication of real test failure

## Comparison

| Aspect | cargo test | cargo nextest |
|--------|-----------|---------------|
| **Execution Model** | Sequential per crate | Parallel per test |
| **Isolation** | Shared process | Separate processes |
| **Output** | Basic | Rich, formatted |
| **Flaky Tests** | Manual retry | Built-in retry |
| **CI Integration** | Standard | Optimized for CI |
| **Local Use** | `cargo test` | `just nt` |
| **Speed** | Baseline | 20-50% faster |

## Known Issues

### Potential Nextest Issues

1. **Installation Time**
   - Nextest needs to be installed first
   - Adds ~30s to CI setup
   - **Mitigation:** One-time cost per run

2. **Different Behavior**
   - Process isolation can expose hidden dependencies
   - Some tests might behave differently
   - **Mitigation:** Fallback to cargo test

3. **Output Format**
   - Nextest output format is different
   - May affect log parsing
   - **Mitigation:** Using `--no-capture` for consistency

### Known Workarounds

Our tests already handle nextest:
- ✅ `NamedTempFile` issues → Manual file management
- ✅ Signal handlers → POSIX `atexit` and `signal`
- ✅ Docker cleanup → Process-unique names

## Monitoring

### Success Criteria

After this change, monitor CI for:

1. ✅ Nextest completes successfully
2. ✅ Fallback is NOT triggered (skipped)
3. ✅ Test execution time improves
4. ✅ No new test failures
5. ✅ Consistent results across runs

### If Nextest Fails

If nextest consistently fails but cargo test passes:

1. Check nextest output for specific errors
2. Identify which tests fail
3. Fix tests for better isolation
4. Consider keeping fallback longer

### When to Remove Fallback

Remove the cargo test fallback when:
- ✅ Nextest works reliably for 2+ weeks
- ✅ No fallback triggers observed
- ✅ Team confident in nextest stability
- ✅ All known issues resolved

## Rollback Plan

If nextest causes problems:

### Option 1: Quick Disable (Keep Fallback)
```yaml
- name: Run tests (nextest - primary)
  continue-on-error: true  # Let it fail gracefully
  run: cargo nextest run ...

- name: Run tests (cargo test - legacy fallback)
  if: always()  # Always run, not just on failure
  run: cargo test -- --nocapture
```

### Option 2: Full Revert
```yaml
# Remove nextest completely
- name: Run tests
  run: cargo test -- --nocapture
```

## Local Development

Developers can continue using either:

```bash
# Nextest (recommended)
just nt

# Or cargo test (still works)
just test
cargo test
```

Both work, nextest is preferred for consistency with CI.

## Future Improvements

### Potential Enhancements

1. **Retry Flaky Tests**
   ```yaml
   cargo nextest run --retries 3
   ```

2. **JUnit Output**
   ```yaml
   cargo nextest run --junit output.xml
   ```

3. **Test Profiles**
   ```toml
   # .config/nextest.toml
   [profile.ci]
   fail-fast = true
   retries = 2
   ```

4. **Caching**
   ```yaml
   - uses: actions/cache@v3
     with:
       path: ~/.cargo/bin/cargo-nextest
   ```

## Related Changes

- `justfile` already has `just nt` command (line 198-201)
- Local dev workflow unchanged
- No code changes required

## Documentation Updates

After confirming stability:
- Update `CONTRIBUTING.md` to mention nextest
- Update `README.md` test section
- Add nextest to prerequisites

## References

- Nextest docs: https://nexte.st/
- GitHub Actions integration: https://nexte.st/docs/ci/github-actions/
- Justfile: `just nt` command

## Summary

- ✅ Added nextest as primary test runner
- ✅ Kept cargo test as fallback (safe migration)
- ✅ Matches local development (`just nt`)
- ✅ Easy rollback if needed
- ✅ Path to future removal of cargo test

**Next Steps:**
1. Monitor CI runs for nextest stability
2. After 2 weeks of success, remove cargo test fallback
3. Optimize nextest configuration for CI

---

**Status:** Phase 1 - Dual Runner (Monitor for stability)

**Goal:** Phase 2 - Nextest Only (Remove fallback in ~2 weeks)

