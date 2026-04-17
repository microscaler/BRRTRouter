# Analysis of Failing SPIFFE JWKS Tests

## Test 1: `test_spiffe_jwks_signature_verification`

### What It's Testing

This test verifies that **JWKS signature verification works end-to-end**:

1. **Setup**:
   - Creates a mock JWKS server with an HS256 key (kid="k1")
   - Creates a `SpiffeProvider` configured with:
     - Trust domain: `example.com`
     - Audience: `api.example.com`
     - JWKS URL pointing to the mock server
   - Creates a properly signed JWT token using the same secret as the JWKS key

2. **Expected Behavior**:
   - When `provider.validate()` is called:
     - Token should be extracted from Authorization header
     - JWT claims should be parsed (without signature verification first)
     - SPIFFE ID should be validated (`spiffe://example.com/api/users`)
     - Trust domain should be validated (`example.com` in whitelist)
     - Audience should be validated (`api.example.com` matches)
     - Expiration should be validated (token not expired)
     - **JWKS should be fetched** (cache is empty, so blocking refresh should occur)
     - **Signature should be verified** using the key from JWKS
     - Validation should return `true`

3. **How It's Failing**:
   - **Assertion at line 1249**: `assert!(result, "Valid signed SPIFFE SVID should pass validation with JWKS...")`
   - The test expects `provider.validate()` to return `true`
   - **Actual result**: `false` (validation is failing)

### Expected Code Flow

```
1. provider.validate() called
2. Token extracted: "Bearer <signed_jwt>"
3. parse_jwt_claims() → extracts claims (sub, aud, exp, iat)
4. SPIFFE ID validation: "spiffe://example.com/api/users" ✓
5. Trust domain check: "example.com" in whitelist ✓
6. Audience check: "api.example.com" matches ✓
7. Expiration check: token not expired ✓
8. JWKS signature verification:
   a. verify_signature() called
   b. Parse JWT header → extract kid="k1"
   c. get_key_for("k1") called
   d. Cache is empty → refresh_jwks_if_needed() triggered
   e. Blocking refresh: HTTP GET to mock server
   f. JWKS response parsed → key "k1" extracted
   g. Key stored in cache
   h. get_key_for() returns DecodingKey
   i. jsonwebtoken::decode() verifies signature ✓
9. Return true
```

### Potential Failure Points

1. **JWKS fetch fails**: Mock server not ready, network error, timeout
2. **Key not found in cache**: Refresh didn't complete, key parsing failed
3. **Signature verification fails**: Wrong key, token malformed, algorithm mismatch
4. **Early validation failure**: SPIFFE ID, trust domain, or audience validation fails before signature check

---

## Test 2: `test_spiffe_jwks_cache_refresh`

### What It's Testing

This test verifies that **JWKS cache refresh works correctly** when cache expires:

1. **Setup**:
   - Creates a mock JWKS server with an HS256 key
   - Creates a `SpiffeProvider` with:
     - Trust domain: `example.com`
     - Audience: `api.example.com`
     - JWKS URL
     - **Short cache TTL: 1 second** (to test refresh behavior)

2. **Test Steps**:
   - **Step 1**: Validate a token (triggers initial JWKS fetch, should succeed)
   - **Step 2**: Wait 2 seconds (cache expires)
   - **Step 3**: Validate the same token again (should trigger background refresh, should still succeed)

3. **Expected Behavior**:
   - First validation: Cache empty → blocking refresh → validation succeeds
   - After 2 second wait: Cache expired → background refresh triggered
   - Second validation: Should use refreshed cache (or wait for refresh) → validation succeeds

4. **How It's Failing**:
   - **Assertion at line 1505**: `assert!(provider.validate(&scheme, &[], &req), "First token should pass validation...")`
   - The test expects the **first** validation to return `true`
   - **Actual result**: `false` (first validation is failing)

### Expected Code Flow

**First Validation (cache empty)**:
```
1. provider.validate() called
2. All basic validations pass (SPIFFE ID, trust domain, audience, expiration)
3. verify_signature() called
4. get_key_for("k1") called
5. Cache is empty → refresh_jwks_if_needed() triggered
6. Blocking refresh: HTTP GET → JWKS fetched → key stored
7. get_key_for() waits for refresh completion (condition variable)
8. Key retrieved from cache
9. Signature verified ✓
10. Return true
```

**Second Validation (after cache expires)**:
```
1. provider.validate() called
2. All basic validations pass
3. verify_signature() called
4. get_key_for("k1") called
5. Cache expired → refresh_jwks_if_needed() triggers background refresh
6. get_key_for() reads from cache (may be stale, but should work)
7. Signature verified ✓
8. Return true
```

### Potential Failure Points

1. **First validation fails**: Same as Test 1 (JWKS fetch, key lookup, signature verification)
2. **Cache refresh not working**: Background refresh not triggered, refresh fails silently
3. **Stale cache issue**: Cache expired but refresh didn't complete, validation uses stale (empty) cache

---

## Root Cause Hypothesis

Based on the analysis, both tests are failing at the **same point**: the first validation attempt when the cache is empty.

### Most Likely Issues

1. **JWKS Refresh Not Completing**:
   - `refresh_jwks_internal()` is called but fails silently
   - HTTP request to mock server fails (server not ready, timeout)
   - JWKS response parsing fails
   - Keys not being stored in cache

2. **Condition Variable Wait Not Working**:
   - `get_key_for()` waits on condition variable, but refresh completes before wait starts
   - Condition variable notification happens before wait, causing missed wakeup
   - Timeout occurs (5 seconds) but refresh actually failed

3. **Key Not Found After Refresh**:
   - Refresh completes successfully
   - Keys are stored in cache
   - But `get_key_for()` reads cache before keys are written (race condition)
   - Or key ID mismatch (kid in token doesn't match kid in JWKS)

4. **Signature Verification Failing**:
   - Key is found and retrieved correctly
   - But `jsonwebtoken::decode()` fails for some reason
   - Algorithm mismatch, token format issue, or validation config problem

### Debugging Steps Needed

1. Add logging to `refresh_jwks_internal()` to see if it's being called and completing
2. Add logging to `get_key_for()` to see if keys are found in cache
3. Add logging to `verify_signature()` to see which step fails
4. Verify mock server is actually receiving and responding to requests
5. Check if condition variable wait is timing out or missing notifications

### Expected vs Actual

**Expected**: 
- Cache empty → blocking refresh → keys in cache → signature verified → `true`

**Actual (hypothesis)**:
- Cache empty → blocking refresh triggered → refresh fails or doesn't complete → `get_key_for()` returns `None` → signature verification fails → `false`

The failure is likely in the **refresh synchronization** or **HTTP fetch** step, not in the signature verification logic itself.

