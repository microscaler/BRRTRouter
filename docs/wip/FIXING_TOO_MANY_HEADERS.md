# Fixing TooManyHeaders Error

## 🐛 The Problem

**Error**: `failed to parse http request: TooManyHeaders`

**Root Cause**: The `may_minihttp` library has a hardcoded limit on the number of HTTP headers it will accept. Modern browsers send many headers, especially with:
- Security headers (CSP, CORS, etc.)
- Cache headers (If-Modified-Since, ETag, etc.)
- Browser fingerprinting headers
- Extensions adding custom headers

## 🔍 What We Need To See

Before we can fix this, we need visibility:
1. **How many headers** is the browser sending?
2. **Which headers** are being sent?
3. **What is the actual limit** in may_minihttp?

## ⚡ Immediate Solution: Add Logging

### Step 1: Add Request Logging to AppService

Update `src/server/service.rs`:

```rust
impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        // Log raw request info BEFORE parsing
        eprintln!("[request] method={} path={} headers={}", 
            req.method(), 
            req.path(),
            req.headers().len()
        );
        
        // Log all header names (not values, for privacy)
        for (idx, h) in req.headers().iter().enumerate() {
            eprintln!("[header:{}] {}", idx, h.name);
        }
        
        let ParsedRequest {
            method,
            path,
            headers,
            cookies,
            query_params,
            body,
        } = parse_request(req);
        
        // Log parsed request
        eprintln!("[parsed] method={} path={} headers={} cookies={} query_params={}",
            method, path, headers.len(), cookies.len(), query_params.len()
        );
        
        // ... rest of code
    }
}
```

### Step 2: Check may_minihttp Source

The error comes from `may_minihttp`. Check its source:

```bash
# Find may_minihttp in cargo cache
find ~/.cargo -name "may_minihttp*" -type d

# Check the source
grep -r "TooManyHeaders" ~/.cargo/registry/src/*/may_minihttp-*/
```

Likely in `may_minihttp/src/request.rs` or similar, there's:

```rust
const MAX_HEADERS: usize = 32;  // or some low number

if headers.len() > MAX_HEADERS {
    return Err(Error::TooManyHeaders);
}
```

### Step 3: Patch may_minihttp Locally

If we find the limit, we can patch it:

1. **Fork may_minihttp** or use a local copy
2. **Increase the limit** to something reasonable (e.g., 64 or 128)
3. **Use path dependency** in Cargo.toml:

```toml
[dependencies]
may_minihttp = { path = "../may_minihttp_patched" }
```

## 🔧 Better Solution: Catch and Handle Gracefully

Even with more headers allowed, we should handle parse errors gracefully:

```rust
impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        // Try to parse, catch errors
        let parsed = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            parse_request(req)
        })) {
            Ok(parsed) => parsed,
            Err(_) => {
                // Parse failed - return 400
                eprintln!("[error] Failed to parse request - too many headers?");
                res.status_code(400, "Bad Request");
                res.header("Content-Type: application/json");
                res.body_vec(
                    br#"{"error":"Bad Request","message":"Too many headers or malformed request"}"#
                        .to_vec()
                );
                return Ok(());
            }
        };
        
        // Continue with parsed request
        let ParsedRequest { method, path, .. } = parsed;
        // ...
    }
}
```

## 📊 Gather Data First

Before patching, let's see what we're dealing with:

### Test Script

```bash
#!/bin/bash

echo "=== Testing Header Counts ==="

# Simple request (minimal headers)
echo "1. Minimal request:"
curl -v http://localhost:8080/ 2>&1 | grep "^>" | wc -l

# Browser-like request (more headers)
echo "2. Browser-like request:"
curl -v \
  -H "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8" \
  -H "Accept-Language: en-US,en;q=0.9" \
  -H "Accept-Encoding: gzip, deflate, br" \
  -H "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)" \
  -H "Referer: http://localhost:8080/" \
  -H "Connection: keep-alive" \
  -H "Cache-Control: max-age=0" \
  http://localhost:8080/ 2>&1 | grep "^>" | wc -l

# Heavy request (lots of headers)
echo "3. Heavy request:"
curl -v \
  -H "Accept: text/html" \
  -H "Accept-Language: en" \
  -H "Accept-Encoding: gzip" \
  -H "User-Agent: Test" \
  -H "Referer: http://localhost:8080/" \
  -H "Connection: keep-alive" \
  -H "Cache-Control: no-cache" \
  -H "Pragma: no-cache" \
  -H "DNT: 1" \
  -H "Upgrade-Insecure-Requests: 1" \
  -H "X-Custom-1: value" \
  -H "X-Custom-2: value" \
  -H "X-Custom-3: value" \
  -H "X-Custom-4: value" \
  -H "X-Custom-5: value" \
  http://localhost:8080/ 2>&1 | grep "^>" | wc -l
```

## 🎯 Recommended Fix Order

1. ✅ **Add logging** - See what's happening (5 min)
2. ✅ **Find may_minihttp limit** - Know what we're dealing with (10 min)
3. ✅ **Patch or fork may_minihttp** - Increase limit (30 min)
4. ✅ **Add comprehensive observability** - Structured logging with tracing (2 hours)
5. ✅ **Add error handling** - Graceful degradation (30 min)

## 🚀 Quick Workaround (NOW)

While we investigate, use a simpler HTTP client:

```bash
# Instead of browser, test with curl
curl http://localhost:8080/

# Or use an incognito/private browser window (fewer extensions = fewer headers)
```

## 📝 Long-term Solution

Replace `may_minihttp` with a more modern HTTP server:
- `hyper` - Industry standard
- `actix-web` - High performance
- `axum` - Modern, tower-based

But this is a significant refactor.

---

**Status**: 🚧 Investigation Needed  
**Priority**: 🔥 Critical - Blocks browser access  
**Next**: Add logging to see actual header counts  
**Date**: October 9, 2025

