# Vendoring and Patching may_minihttp

## Goal

Fix the `TooManyHeaders` error by increasing the header buffer size in `may_minihttp`.

## Quick Start

```bash
# 1. Run the vendor script
chmod +x scripts/vendor-may-minihttp.sh
./scripts/vendor-may-minihttp.sh

# 2. Find and edit the header buffer
cd vendor/may_minihttp-0.1.11
# Find the file with header allocation (likely src/request.rs or src/lib.rs)
grep -rn "EMPTY_HEADER\|httparse::Header" src/

# 3. Edit the file and change the buffer size from ~16/32 to 128
# Example: Change this line:
#   let mut headers = [httparse::EMPTY_HEADER; 16];
# To:
#   let mut headers = [httparse::EMPTY_HEADER; 128];

# 4. Add patch to Cargo.toml (see below)

# 5. Test
cargo build
cargo test
```

## Step-by-Step Guide

### Step 1: Download may_minihttp Source

```bash
# Create vendor directory
mkdir -p vendor
cd vendor

# Download from crates.io
curl -L "https://crates.io/api/v1/crates/may_minihttp/0.1.11/download" -o may_minihttp.tar.gz
tar xzf may_minihttp.tar.gz
rm may_minihttp.tar.gz

# You should now have: vendor/may_minihttp-0.1.11/
```

**Alternative: Clone from GitHub**

```bash
cd vendor
git clone https://github.com/Xudong-Huang/may_minihttp.git
cd may_minihttp
git checkout v0.1.11  # or the version you need
```

### Step 2: Find the Header Buffer

The header buffer is typically defined in `src/request.rs` or `src/lib.rs`. Look for:

```rust
// Common patterns:
let mut headers = [httparse::EMPTY_HEADER; 16];
const MAX_HEADERS: usize = 32;
let mut headers: [Header; 16] = Default::default();
```

**Search command:**

```bash
cd vendor/may_minihttp-0.1.11
grep -rn "EMPTY_HEADER\|MAX_HEADER\|httparse::Header" src/
```

**Example output might show:**

```
src/request.rs:42:    let mut headers = [httparse::EMPTY_HEADER; 16];
src/request.rs:43:    let mut req = httparse::Request::new(&mut headers);
```

This tells us:
- File: `src/request.rs`
- Line: 42
- Current buffer size: **16 headers**

### Step 3: Increase the Buffer Size

Edit the file (e.g., `src/request.rs`):

```rust
// BEFORE:
let mut headers = [httparse::EMPTY_HEADER; 16];

// AFTER:
let mut headers = [httparse::EMPTY_HEADER; 128];
```

**Recommended buffer sizes:**
- **16**: Original (too small for modern web apps)
- **32**: Minimal improvement
- **64**: Good for most cases
- **128**: Recommended (handles complex scenarios)
- **256**: Overkill but safe

**Why 128?**
- Kubernetes probe headers: ~5
- Browser headers: ~15
- Load balancer headers: ~10
- Auth/security headers: ~10
- Cookies: ~5-10
- **Total**: ~45-50 typical, 128 provides 2.5x safety margin

### Step 4: Add Patch to Cargo.toml

In your project root `Cargo.toml`, add:

```toml
[patch.crates-io]
may_minihttp = { path = "vendor/may_minihttp-0.1.11" }
```

**Full example:**

```toml
[package]
name = "brrtrouter"
version = "0.1.0-alpha.1"
edition = "2021"

[dependencies]
may_minihttp = "0.1.11"
# ... other deps ...

# At the bottom of the file:
[patch.crates-io]
may_minihttp = { path = "vendor/may_minihttp-0.1.11" }
```

This tells Cargo: "When any crate (including transitively) depends on `may_minihttp` from crates.io, use my local version instead."

### Step 5: Verify the Patch

```bash
# Clean build to ensure patch is used
cargo clean

# Build
cargo build

# Check that it's using the vendored version
cargo tree -i may_minihttp

# Should show:
#   may_minihttp v0.1.11 (vendor/may_minihttp-0.1.11)
#   (not: may_minihttp v0.1.11 (crates.io))
```

### Step 6: Test the Fix

**Test 1: Local curl with many headers**

```bash
# Start the server
cargo run --bin pet_store -- --spec examples/openapi.yaml --port 8080 &
SERVER_PID=$!
sleep 2

# Test with progressively more headers
for count in 10 20 30 40 50 60 70 80; do
    echo -n "Testing with $count headers: "
    
    headers=""
    for i in $(seq 1 $count); do
        headers="$headers -H \"X-Test-$i: value$i\""
    done
    
    response=$(eval curl -s -w '%{http_code}' -o /dev/null $headers http://localhost:8080/health)
    
    if [ "$response" = "200" ]; then
        echo "✅ OK"
    else
        echo "❌ FAILED (HTTP $response)"
        break
    fi
done

# Cleanup
kill $SERVER_PID
```

**Test 2: Check Tilt logs**

```bash
# Deploy with Tilt
tilt up

# Watch for TooManyHeaders errors
tilt logs petstore | grep -i "TooManyHeaders"

# If no errors appear after a few minutes of traffic, it's fixed!
```

**Test 3: Generate realistic traffic**

```bash
# Send requests with realistic header counts
for i in {1..100}; do
    curl -H "User-Agent: Mozilla/5.0" \
         -H "Accept: application/json" \
         -H "Accept-Language: en-US,en;q=0.9" \
         -H "Accept-Encoding: gzip, deflate, br" \
         -H "Connection: keep-alive" \
         -H "X-Forwarded-For: 10.0.0.1" \
         -H "X-Forwarded-Proto: https" \
         -H "X-Real-IP: 10.0.0.1" \
         -H "X-Request-ID: req-$i" \
         -H "Authorization: Bearer token123" \
         http://localhost:9090/health
    sleep 0.1
done

# Check logs
tilt logs petstore --tail=50 | grep -i "TooManyHeaders"
```

## Documenting the Change

Add a comment in the patched file explaining why:

```rust
// PATCHED for BRRTRouter:
// Increased from 16 to 128 to handle modern web applications with:
// - Kubernetes probe headers
// - Load balancer headers (X-Forwarded-*, X-Real-IP)
// - Browser headers (User-Agent, Accept-*, Cookie, etc.)
// - Auth headers (Authorization, X-API-Key, etc.)
// See: https://github.com/your-org/BRRTRouter/docs/VENDORING_MAY_MINIHTTP.md
let mut headers = [httparse::EMPTY_HEADER; 128];
```

## Alternative: Contribute Upstream

Instead of maintaining a local patch, consider contributing back:

### Option 1: Make Buffer Configurable

```rust
// Propose adding a const that can be configured
pub const DEFAULT_MAX_HEADERS: usize = 64;

// Allow users to configure via feature flags
#[cfg(feature = "large-headers")]
pub const MAX_HEADERS: usize = 256;

#[cfg(not(feature = "large-headers"))]
pub const MAX_HEADERS: usize = DEFAULT_MAX_HEADERS;

// Use in code:
let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
```

### Option 2: Dynamic Allocation

```rust
// Instead of fixed array, use Vec
let max_headers = std::env::var("MAX_HTTP_HEADERS")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(64);

let mut headers = vec![httparse::EMPTY_HEADER; max_headers];
```

### Submit PR

1. Fork `may_minihttp` on GitHub
2. Create branch: `git checkout -b increase-header-buffer`
3. Make changes (preferably configurable)
4. Add tests
5. Update README/docs
6. Submit PR with rationale:

```markdown
## Increase Default Header Buffer Size

### Problem
Modern web applications commonly send 20-40 headers due to:
- Security headers (CSP, CORS, etc.)
- Load balancers (X-Forwarded-*, X-Real-IP)
- CDNs and proxies
- Authentication (cookies, tokens)
- Monitoring/tracing (X-Request-ID, trace IDs)

Current limit of 16 headers causes `TooManyHeaders` errors.

### Solution
Increase default buffer to 64 headers (4x current).
This matches common HTTP server defaults (Nginx: 100, Apache: unlimited).

### Performance Impact
Memory: +48 bytes per request (negligible)
Speed: No change (fixed-size array)

### Testing
Tested with 100 headers, all parse successfully.
```

## Maintaining the Patch

### When Updating may_minihttp

```bash
# 1. Check for new version
cargo outdated | grep may_minihttp

# 2. Download new version
cd vendor
curl -L "https://crates.io/api/v1/crates/may_minihttp/0.1.12/download" -o may_minihttp-new.tar.gz
tar xzf may_minihttp-new.tar.gz

# 3. Apply your patch again
cd may_minihttp-0.1.12
# Re-apply the buffer size change

# 4. Update Cargo.toml patch path
# Change: may_minihttp = { path = "vendor/may_minihttp-0.1.11" }
# To:     may_minihttp = { path = "vendor/may_minihttp-0.1.12" }

# 5. Test
cd ../..
cargo clean
cargo build
cargo test
```

### Keep Patch Minimal

Only change what's necessary:
- ✅ Increase buffer size
- ✅ Add comment explaining change
- ❌ Don't refactor other code
- ❌ Don't change formatting
- ❌ Don't update dependencies

This makes merging upstream changes easier.

## Troubleshooting

### Patch Not Being Used

```bash
# Check cargo is finding the patch
cargo tree -i may_minihttp

# Should show path, not crates.io:
# may_minihttp v0.1.11 (vendor/may_minihttp-0.1.11)
```

If still using crates.io version:
1. Check `[patch.crates-io]` is in root `Cargo.toml` (not workspace member)
2. Run `cargo clean`
3. Delete `Cargo.lock` and run `cargo build`

### Still Getting TooManyHeaders

Possible causes:
1. **Patch not applied** - Check `cargo tree -i may_minihttp`
2. **Wrong file edited** - may_minihttp might parse headers in multiple places
3. **Old binary running** - Restart the service
4. **Headers exceed new limit** - Increase further or add logging to count actual headers

### Build Errors After Patch

```bash
# Make sure the vendored version compiles standalone
cd vendor/may_minihttp-0.1.11
cargo build

# Check for syntax errors in your edits
cargo check
```

### Performance Concerns

**Q: Does a larger buffer slow things down?**
A: No. It's a fixed-size array on the stack, allocated once per request.

**Q: How much memory does this use?**
A: Each `httparse::Header` is ~48 bytes. 
   - 16 headers = 768 bytes
   - 128 headers = 6,144 bytes
   - Difference = 5,376 bytes (~5KB per request)

For reference, coroutine stacks are 64KB by default, so 5KB is only 8% overhead.

## Summary

✅ **Download**: `may_minihttp` source to `vendor/`  
✅ **Find**: Header buffer allocation (usually `src/request.rs`)  
✅ **Change**: Buffer size from 16/32 to 128  
✅ **Patch**: Add `[patch.crates-io]` to `Cargo.toml`  
✅ **Test**: Send requests with many headers  
✅ **Document**: Add comment explaining change  
✅ **Contribute**: Consider upstream PR  

**Files:**
- `vendor/may_minihttp-0.1.11/src/request.rs` (or similar) - The actual fix
- `Cargo.toml` - Patch directive
- `docs/VENDORING_MAY_MINIHTTP.md` - This guide
- `scripts/vendor-may-minihttp.sh` - Helper script

**Result**: No more `TooManyHeaders` errors! 🎉

