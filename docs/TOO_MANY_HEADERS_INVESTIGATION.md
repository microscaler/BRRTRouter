# TooManyHeaders Error Investigation

## Root Cause Identified ✅

The `TooManyHeaders` error is **NOT** from BRRTRouter code - it's from the `httparse` crate used by `may_minihttp`.

### Dependency Chain

```
BRRTRouter
  └─> may_minihttp (v0.1.11)
       └─> httparse (v1.10.1)  ← Error originates here
```

### What's Happening

1. **`httparse` requires a header buffer** when parsing HTTP requests
2. **`may_minihttp` allocates this buffer** with a fixed size
3. **When too many headers arrive**, `httparse` returns `Error::TooManyHeaders`
4. **`may_minihttp` logs the error** to stderr: `"failed to parse http request: TooManyHeaders"`

## From the Logs

```
2025-10-10T10:35:32.277801843Z stderr F failed to parse http request: TooManyHeaders
2025-10-10T10:35:26.250409091Z stderr F failed to parse http request: TooManyHeaders
2025-10-10T10:35:24.217688215Z stderr F failed to parse http request: TooManyHeaders
```

These errors are happening repeatedly, likely from:
- Kubernetes liveness/readiness probes with many headers
- Browser requests with extensive headers (cookies, security headers, etc.)
- Load balancer/ingress adding headers

## Typical Header Counts

**Normal browser request**: 8-15 headers
**Kubernetes probe**: 3-5 headers
**Behind load balancer**: 15-25 headers (adds X-Forwarded-*, X-Real-IP, etc.)
**With auth/cookies**: 20-30 headers

**Common default buffer sizes**:
- `httparse` examples: 16 headers
- `hyper`: 100 headers
- `actix-web`: 32 headers
- **`may_minihttp`**: Unknown (need to check source)

## Investigating `may_minihttp`

### Check the source code

```bash
# Find where may_minihttp is cached
find ~/.cargo/registry/src -name "may_minihttp*" -type d

# Look for the header buffer allocation
grep -r "httparse\|Header\|TooManyHeaders" ~/.cargo/registry/src/*/may_minihttp-*/

# Common patterns to look for:
# - let mut headers = [httparse::Header::default(); 16];  ← Buffer of 16
# - const MAX_HEADERS: usize = 32;
# - headers: &mut [Header<'_>]
```

### Example from typical HTTP server

```rust
// Common pattern in Rust HTTP servers:
let mut headers = [httparse::EMPTY_HEADER; 64];  // 64 header buffer
let mut req = httparse::Request::new(&mut headers);

match req.parse(buf) {
    Ok(status) => { /* ... */ },
    Err(httparse::Error::TooManyHeaders) => {
        eprintln!("Request has more than {} headers", headers.len());
    }
}
```

## Solutions (In Order of Preference)

### Option 1: Patch `may_minihttp` (BEST)

Create a local patch to increase the header buffer:

```toml
# Cargo.toml
[patch.crates-io]
may_minihttp = { path = "vendor/may_minihttp" }
```

Then fork/vendor `may_minihttp` and change:
```rust
// From:
let mut headers = [httparse::EMPTY_HEADER; 16];  // or whatever it is

// To:
let mut headers = [httparse::EMPTY_HEADER; 128]; // Much larger
```

**Pros**: 
- Fixes root cause
- No runtime overhead
- Clean solution

**Cons**: 
- Need to maintain fork/patch
- Need to verify with each `may_minihttp` update

### Option 2: Contribute Upstream (IDEAL)

1. Fork `may_minihttp` on GitHub
2. Add configurable `MAX_HEADERS` const
3. Default to 64 or 128 headers
4. Submit PR with benchmarks
5. Wait for merge

**Pros**: 
- Helps entire community
- No maintenance burden

**Cons**: 
- Takes time for PR review/merge
- Might not be accepted

### Option 3: Switch HTTP Server Library

Replace `may_minihttp` with a more robust HTTP server:

**Options**:
- `hyper` (most popular, 100 header default)
- `actix-web` (fast, 32 header default)
- `axum` (modern, built on hyper)
- `tide` (simple, good defaults)

**Pros**: 
- Better maintained
- More features
- Larger header buffers

**Cons**: 
- Major refactor needed
- May lose `may` coroutine integration
- Breaking change for users

### Option 4: Suppress/Handle Error Gracefully

In `src/server/service.rs`, handle the error better:

```rust
impl HttpService for AppService {
    fn call(&mut self, req: Request, rsp: &mut Response) -> io::Result<()> {
        // Wrap the entire call in error handling
        match self.call_inner(req, rsp) {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("TooManyHeaders") => {
                // Log once per minute to avoid spam
                static LAST_LOG: OnceLock<AtomicU64> = OnceLock::new();
                let last = LAST_LOG.get_or_init(|| AtomicU64::new(0));
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let prev = last.load(Ordering::Relaxed);
                
                if now - prev > 60 {
                    eprintln!("[WARN] TooManyHeaders error (rate limited log)");
                    last.store(now, Ordering::Relaxed);
                }
                
                // Return 431 Request Header Fields Too Large
                rsp.status_line("HTTP/1.1 431 Request Header Fields Too Large");
                rsp.header("Content-Type", "text/plain");
                rsp.body("Request has too many headers\n");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
```

**Pros**: 
- Quick fix
- Reduces log spam
- Returns proper HTTP error

**Cons**: 
- Doesn't fix root cause
- Requests still fail
- Band-aid solution

## Immediate Actions

### 1. Add Comprehensive Logging

We've already added panic handlers to `main.rs`. Now let's add detailed request logging to understand the header patterns:

**Goal**: See exactly how many headers are in failing requests

### 2. Check `may_minihttp` Source

```bash
# Find the exact header buffer size
cd ~/.cargo/registry/src/github.com-*/may_minihttp-*
grep -n "EMPTY_HEADER\|Header.*default\|MAX_HEADER" src/*.rs
```

Look for lines like:
```rust
const MAX_HEADERS: usize = 16;
let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
```

### 3. Test with Known Header Count

Create a test that sends progressively more headers:

```bash
# Test with 10 headers
curl -H "X-Test-1: 1" -H "X-Test-2: 2" ... http://localhost:9090/health

# Test with 20 headers
# ... etc
```

Find the exact threshold where it fails.

### 4. Decide on Solution

Based on findings:
- If limit is < 32: **Definitely needs patching**
- If limit is 32-64: **Maybe acceptable, but should increase**
- If limit is > 64: **Investigate why so many headers**

## Expected Findings

Most likely scenario:
- `may_minihttp` uses 16 or 32 header buffer
- Kubernetes probes + browser headers + load balancer headers = 20-30 total
- Occasionally exceeds limit, causing errors

## Recommended Path Forward

1. ✅ **Add logging** (completed with panic handler)
2. 🔄 **Investigate `may_minihttp` source** (next step)
3. 🔄 **Create local patch** with larger buffer (128 headers)
4. 🔄 **Test thoroughly**
5. 🔄 **Submit upstream PR** to help community
6. 📋 **Document in README** as known issue if upstream not fixed

## Questions to Answer

1. **What's the current header limit?** (Check `may_minihttp` source)
2. **How many headers are typical requests sending?** (Add logging)
3. **Is this from probes or real traffic?** (Check request patterns)
4. **Can we reproduce locally?** (Send many headers via curl)

## Files to Create/Modify

1. `vendor/may_minihttp/` - Local patch (if we go that route)
2. `src/server/service.rs` - Better error handling
3. `docs/KNOWN_ISSUES.md` - Document the limitation
4. `Cargo.toml` - Add patch directive if needed

## Testing Plan

```bash
# 1. Find current limit
for i in {1..100}; do
  headers=""
  for j in $(seq 1 $i); do
    headers="$headers -H 'X-Test-$j: value$j'"
  done
  
  response=$(eval curl -s -w '%{http_code}' -o /dev/null $headers http://localhost:9090/health)
  if [ "$response" != "200" ]; then
    echo "Failed at $i headers"
    break
  fi
done

# 2. Check Kubernetes probe headers
kubectl exec -n brrtrouter-dev -l app=petstore -- \
  env | grep -i http

# 3. Capture real request headers
tcpdump -i any -A 'tcp port 8080' | grep -A 50 "GET\|POST"
```

## Status

- ✅ Root cause identified: `httparse` via `may_minihttp`
- ✅ Error location confirmed: `may_minihttp` parsing layer
- ✅ Panic handler added to catch crashes
- 🔄 **Next**: Investigate `may_minihttp` source for exact limit
- 🔄 **Next**: Add header count logging
- 🔄 **Next**: Test with varying header counts
- 🔄 **Next**: Create patch or contribute upstream

## References

- [httparse docs](https://docs.rs/httparse/latest/httparse/)
- [httparse Error::TooManyHeaders](https://docs.rs/httparse/latest/httparse/enum.Error.html#variant.TooManyHeaders)
- [may_minihttp on crates.io](https://crates.io/crates/may_minihttp)
- [RFC 7230 - HTTP/1.1 Message Syntax](https://tools.ietf.org/html/rfc7230)

