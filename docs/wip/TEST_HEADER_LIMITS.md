# Testing Header Limits

## Quick Test

```bash
# Run the comprehensive header limit test
just test-headers
```

This will:
1. ✅ Verify the patch is applied (`MAX_HEADERS = 128`)
2. ✅ Build with patched source
3. ✅ Start test server on port 18080
4. ✅ Test with 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120 headers
5. ✅ Test with realistic browser/K8s headers (30+ headers)
6. ✅ Check logs for `TooManyHeaders` errors
7. ✅ Report results

## Expected Output

```
🧪 Testing Header Limits After Patch
======================================

📦 Step 1: Verify patch is applied...
✅ Patch confirmed: MAX_HEADERS = 128

🔨 Step 2: Building with patched source...
✅ Using vendored may_minihttp

🚀 Step 3: Starting test server...
   Server PID: 12345
   Waiting for server to be ready ✓

🧪 Step 4: Testing with progressively more headers...

✅ 10 headers: OK
✅ 20 headers: OK
✅ 30 headers: OK
✅ 40 headers: OK
✅ 50 headers: OK
✅ 60 headers: OK
✅ 70 headers: OK
✅ 80 headers: OK
✅ 90 headers: OK
✅ 100 headers: OK
✅ 110 headers: OK
✅ 120 headers: OK

🌐 Step 5: Testing with realistic browser/K8s headers...
✅ Realistic browser request (30+ headers): OK

🔍 Step 6: Checking server logs for errors...
✅ No TooManyHeaders errors in logs
✅ No panics or fatal errors

🧹 Step 7: Cleanup...
   Server stopped

======================================
📊 TEST RESULTS
======================================

✅ Passed: 13 tests
❌ Failed: 0 tests
🎯 Max successful headers: 120

╔════════════════════════════════════╗
║   🎉 ALL TESTS PASSED! 🎉          ║
║                                    ║
║   TooManyHeaders is FIXED!         ║
║   Supports 100+ headers            ║
╚════════════════════════════════════╝
```

## Manual Testing

### Test 1: Simple Header Test

```bash
# Build
cargo build --release -p pet_store

# Start server
cargo run --release -p pet_store -- \
  --spec examples/pet_store/doc/openapi.yaml \
  --port 8080 &

# Test with 50 headers (would fail with original 16 limit)
headers=""
for i in {1..50}; do
  headers="$headers -H 'X-Test-$i: value$i'"
done
eval curl -v $headers http://localhost:8080/health

# Should return: HTTP/1.1 200 OK
# NOT: failed to parse http request: TooManyHeaders

# Cleanup
pkill -f pet_store
```

### Test 2: Realistic Browser Request

```bash
# Start server
cargo run --release -p pet_store -- \
  --spec examples/pet_store/doc/openapi.yaml \
  --port 8080 &

# Send request with typical browser headers
curl -v \
  -H "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)" \
  -H "Accept: application/json" \
  -H "Accept-Language: en-US,en;q=0.9" \
  -H "Accept-Encoding: gzip, deflate, br" \
  -H "Connection: keep-alive" \
  -H "X-Forwarded-For: 10.0.0.1" \
  -H "X-Forwarded-Proto: https" \
  -H "X-Real-IP: 10.0.0.1" \
  -H "X-Request-ID: test-123" \
  -H "Authorization: Bearer token123" \
  -H "X-API-Key: test123" \
  -H "Cookie: session=abc; user=test" \
  http://localhost:8080/health

# Should work perfectly now!

# Cleanup
pkill -f pet_store
```

### Test 3: In Tilt (Kubernetes)

```bash
# Deploy with Tilt
tilt up

# Wait for service to start
kubectl wait --for=condition=ready pod -l app=petstore -n brrtrouter-dev --timeout=60s

# Send requests with many headers
for i in {1..100}; do
  curl -s \
    -H "User-Agent: TestClient/$i" \
    -H "Accept: application/json" \
    -H "X-Request-ID: req-$i" \
    -H "X-Trace-ID: trace-$i" \
    -H "X-Forwarded-For: 10.0.$((i % 256)).$((i % 256))" \
    -H "Authorization: Bearer token$i" \
    http://localhost:9090/health > /dev/null
  
  if [ $((i % 10)) -eq 0 ]; then
    echo "Sent $i requests..."
  fi
done

echo "All requests sent!"

# Check for errors
echo ""
echo "Checking for TooManyHeaders errors..."
tilt logs petstore --since=5m | grep -i "TooManyHeaders" || \
  echo "✅ No TooManyHeaders errors!"
```

## What Gets Tested

### Header Counts
- **10 headers**: Minimal request
- **20 headers**: Typical browser
- **30 headers**: Browser + load balancer
- **40 headers**: Browser + LB + auth
- **50 headers**: Complex SPA
- **60-120 headers**: Extreme but supported

### Realistic Scenarios
- Kubernetes health probes
- Browser requests with cookies
- Requests behind load balancer/CDN
- Requests with auth tokens
- Requests with tracking headers

## Troubleshooting Test Failures

### If Tests Fail

**1. Check patch is applied:**
```bash
grep "const MAX_HEADERS" vendor/may_minihttp/src/request.rs
# Should show: pub(crate) const MAX_HEADERS: usize = 128;
```

**2. Check vendored source is being used:**
```bash
cargo clean
cargo build -v 2>&1 | grep "may_minihttp"
# Should show: Compiling may_minihttp ... (vendor/may_minihttp)
```

**3. Check .cargo/config.toml:**
```bash
cat .cargo/config.toml | grep -A 3 "vendored-sources"
# Should show the vendored-sources configuration
```

**4. Clean rebuild:**
```bash
cargo clean
rm -rf target/
rm Cargo.lock
cargo build --release -p pet_store
just test-headers
```

### Port Already in Use

If you get "Address already in use" error:

```bash
# Find what's using port 18080
lsof -i :18080

# Kill it
kill -9 <PID>

# Or use a different port
# Edit the script to use a different port
```

### Server Won't Start

Check the log file:
```bash
tail -50 /tmp/test-server.log
```

Common issues:
- Missing openapi.yaml
- Missing config.yaml
- Port already in use
- Binary not built

## CI Integration

The test script is designed to work in CI:

```yaml
# .github/workflows/ci.yml
- name: Test header limits
  run: |
    just test-headers
```

Exit codes:
- `0`: All tests passed
- `1`: Some tests failed or max < 100 headers

## Performance Impact

**Memory per request:**
- Before (16 headers): 768 bytes
- After (128 headers): 6,144 bytes
- **Difference: +5.4KB** (0.008% of 64KB coroutine stack)

**Performance:**
- No measurable impact
- Still stack-allocated fixed array
- No dynamic allocations

## Summary

✅ Quick test: `just test-headers`  
✅ Tests 10-120 headers  
✅ Tests realistic scenarios  
✅ Checks for errors in logs  
✅ Clear pass/fail reporting  
✅ CI-friendly with exit codes  

**If all tests pass, the TooManyHeaders issue is completely fixed!** 🎉

