# CORS, JWKS, and SPIFFE: Holistic System Audit

## Executive Summary

This audit examines the three interconnected security systems in BRRTRouter:
- **CORS** (Cross-Origin Resource Sharing) - Always required, even if relaxed
- **JWKS** (JSON Web Key Set) - Can be used independently without SPIFFE
- **SPIFFE** (Secure Production Identity Framework) - Requires JWKS for signature verification

The systems are "screws on the same machine" and must be considered holistically for interoperability.

---

## 1. System Architecture Overview

### 1.1 CORS Middleware (`src/middleware/cors/`)

**Purpose**: Handle cross-origin requests, preflight OPTIONS requests, and add CORS headers to responses.

**Key Characteristics**:
- **Always Required**: CORS middleware should always be present, even if configured permissively
- **Request Flow**: Handles requests in middleware chain (before handler execution)
- **Configuration Sources**:
  - Global: `config.yaml` (origins, headers, methods, credentials)
  - Route-specific: OpenAPI `x-cors` extension (overrides global settings)
- **Policy Types**:
  - `Inherit`: Use global CORS configuration
  - `Disabled`: No CORS headers for this route
  - `Custom`: Route-specific CORS configuration

**Files**:
- `src/middleware/cors/mod.rs` - Core CORS middleware
- `src/middleware/cors/builder.rs` - Builder pattern for configuration
- `src/middleware/cors/route_config.rs` - Route-specific CORS policies
- `src/middleware/cors/error.rs` - CORS configuration errors

### 1.2 JWKS Bearer Provider (`src/security/jwks_bearer/`)

**Purpose**: Standalone JWT validation using JSON Web Key Sets from external identity providers.

**Key Characteristics**:
- **Independent**: Works without SPIFFE - designed for Auth0, Okta, etc.
- **JWKS URL Required**: Must be configured at initialization
- **Caching**: In-memory cache with TTL, background refresh
- **Algorithms**: Supports HS256/384/512, RS256/384/512
- **Claims Validation**: Validates `exp`, `iss`, `aud`, `scope`

**Files**:
- `src/security/jwks_bearer/mod.rs` - Provider implementation
- `src/security/jwks_bearer/validation.rs` - JWT validation logic

**Dependencies**: None on SPIFFE

### 1.3 SPIFFE Provider (`src/security/spiffe/`)

**Purpose**: Validate SPIFFE JWT SVIDs (SPIFFE Verifiable Identity Documents) for service identity.

**Key Characteristics**:
- **Requires JWKS**: JWKS URL is **mandatory** for signature verification (security requirement)
- **SPIFFE-Specific Validation**:
  - SPIFFE ID format (`spiffe://trust-domain/path`)
  - Trust domain whitelist
  - Audience validation
  - Algorithm mismatch validation (token algorithm must match JWKS key algorithm)
- **Additional Features**:
  - Token revocation checking (optional)
  - Cookie-based token extraction (optional)

**Files**:
- `src/security/spiffe/mod.rs` - Provider implementation
- `src/security/spiffe/validation.rs` - SPIFFE-specific validation logic
- `src/security/spiffe/revocation.rs` - Token revocation support

**Dependencies**: Requires JWKS for signature verification

---

## 2. Dependency Graph

```
┌─────────────────┐
│   CORS          │  Always Required (even if relaxed)
│   Middleware    │  No dependencies on JWKS/SPIFFE
└─────────────────┘
        │
        │ (handles preflight, adds headers)
        ▼
┌─────────────────┐
│   Request       │
│   Processing    │
└─────────────────┘
        │
        ├──────────────────┐
        │                  │
        ▼                  ▼
┌──────────────┐   ┌──────────────┐
│   JWKS       │   │   SPIFFE     │
│   Bearer     │   │   Provider   │
│   Provider   │   │              │
│              │   │  (requires   │
│  (standalone)│   │   JWKS)      │
└──────────────┘   └──────────────┘
        │                  │
        │                  │ (uses JWKS for signature verification)
        │                  │
        └──────────────────┘
                 │
                 ▼
          ┌──────────────┐
          │   Security   │
          │   Validation│
          └──────────────┘
```

### 2.1 Dependency Rules

1. **CORS → No Dependencies**
   - CORS middleware operates independently
   - Handles preflight requests before security validation
   - Adds headers after handler execution

2. **JWKS → No Dependencies**
   - `JwksBearerProvider` is completely independent
   - No SPIFFE code in `src/security/jwks_bearer/`
   - Can be used for any JWKS-based JWT validation

3. **SPIFFE → Requires JWKS**
   - `SpiffeProvider` requires `jwks_url` to be configured
   - Validation fails if JWKS URL is not set (fail-secure)
   - Uses JWKS cache for signature verification
   - Validates algorithm mismatch (token algorithm must match JWKS key algorithm)

---

## 3. Current State Analysis

### 3.1 CORS Implementation

**Status**: ✅ Well-implemented

**Strengths**:
- Global configuration from `config.yaml`
- Route-specific overrides via OpenAPI `x-cors` extension
- Proper handling of preflight OPTIONS requests
- Credentials validation (wildcard + credentials = panic)

**Configuration Flow**:
1. Load global CORS config from `config.yaml` at startup
2. Extract route-specific CORS policies from OpenAPI spec
3. Merge origins from config.yaml into route-specific configs
4. Create `CorsMiddleware` with merged policies
5. All processing at startup (no runtime allocations)

**Test Coverage**: `tests/auth_cors_tests.rs`, `tests/middleware_tests.rs`

### 3.2 JWKS Bearer Provider

**Status**: ✅ Well-implemented, Independent

**Strengths**:
- Completely independent of SPIFFE
- Production-ready with caching and background refresh
- Supports multiple algorithms
- Claims caching for performance

**Key Features**:
- JWKS URL required at initialization
- Automatic key rotation support
- Sub-second cache TTL support
- Claims cache with LRU eviction

**Test Coverage**: `tests/security_tests.rs` (JWKS-specific tests)

**No SPIFFE Dependencies**: ✅ Verified - grep shows no SPIFFE references

### 3.3 SPIFFE Provider

**Status**: ✅ Well-implemented, Requires JWKS

**Strengths**:
- Proper security enforcement (JWKS URL required)
- Algorithm mismatch validation (prevents algorithm confusion attacks)
- Trust domain and audience validation
- Token revocation support

**Recent Security Fixes**:
1. **JWKS URL Required**: Validation fails if `jwks_url` is `None` (fail-secure)
2. **Algorithm Mismatch Validation**: Token algorithm must match JWKS key algorithm
3. **Signature Verification**: Always performed when JWKS URL is configured

**Test Coverage**: `tests/spiffe_tests.rs`

**JWKS Dependency**: ✅ Verified - requires `jwks_url` configuration

---

## 4. Interoperability Analysis

### 4.1 CORS + JWKS Interaction

**Current State**: ✅ No direct interaction
- CORS handles preflight and headers
- JWKS validates JWT tokens
- Both operate independently

**Potential Issues**: None identified

### 4.2 CORS + SPIFFE Interaction

**Current State**: ✅ No direct interaction
- CORS handles preflight and headers
- SPIFFE validates SPIFFE SVIDs
- Both operate independently

**Potential Issues**: None identified

### 4.3 JWKS + SPIFFE Interaction

**Current State**: ✅ Proper dependency
- SPIFFE requires JWKS URL (enforced)
- SPIFFE uses JWKS cache for signature verification
- Algorithm mismatch validation prevents confusion attacks

**Key Implementation Details**:
- SPIFFE stores `(DecodingKey, Algorithm)` in cache (not just `DecodingKey`)
- Validates token algorithm matches JWKS key algorithm before signature verification
- This prevents algorithm confusion attacks where attacker uses different algorithm

**Potential Issues**: None identified

---

## 5. Test Coverage Analysis

### 5.1 CORS Tests

**Location**: `tests/auth_cors_tests.rs`, `tests/middleware_tests.rs`

**Coverage**:
- ✅ CORS header setting
- ✅ Preflight OPTIONS handling
- ✅ Origin validation
- ✅ Credentials handling
- ✅ Route-specific CORS policies

**Gaps**: None identified

### 5.2 JWKS Tests

**Location**: `tests/security_tests.rs`

**Coverage**:
- ✅ JWKS fetching and caching
- ✅ Key rotation
- ✅ Claims validation
- ✅ Cache TTL and refresh
- ✅ Background refresh thread
- ✅ Sub-second cache TTL

**Gaps**: None identified

### 5.3 SPIFFE Tests

**Location**: `tests/spiffe_tests.rs`

**Coverage**:
- ✅ SPIFFE ID format validation
- ✅ Trust domain validation
- ✅ Audience validation
- ✅ Signature verification (with JWKS)
- ✅ Algorithm mismatch validation
- ✅ Token revocation
- ✅ Cookie extraction
- ✅ Expiration and leeway

**Recent Fixes**:
- ✅ All tests updated to use signed tokens with JWKS configured
- ✅ Edge cases for audience validation (object, number, boolean, null, missing)
- ✅ Algorithm mismatch validation test

**Gaps**: None identified

### 5.4 Integration Tests

**Location**: `tests/auth_cors_tests.rs`

**Coverage**:
- ✅ CORS + Auth middleware interaction
- ✅ CORS headers on authenticated requests

**Gaps**: 
- ⚠️ No tests for CORS + JWKS interaction
- ⚠️ No tests for CORS + SPIFFE interaction
- ⚠️ No tests for JWKS + SPIFFE interoperability scenarios

---

## 6. Configuration Flow Analysis

### 6.1 Startup Configuration

**CORS**:
1. Load `config.yaml` → Extract `cors` section
2. Build global `CorsMiddleware` from config
3. Extract route-specific CORS from OpenAPI `x-cors` extension
4. Merge origins from config.yaml into route-specific configs
5. Create `CorsMiddleware` with merged policies

**JWKS**:
1. Initialize `JwksBearerProvider` with JWKS URL
2. Fetch initial JWKS keys
3. Start background refresh thread
4. Register with `AppService` via `register_security_provider()`

**SPIFFE**:
1. Initialize `SpiffeProvider` with trust domains and audiences
2. **Require** `jwks_url` configuration
3. Initialize JWKS cache (same structure as JWKS provider)
4. Fetch initial JWKS keys
5. Register with `AppService` via `register_security_provider()`

### 6.2 Runtime Flow

```
Request Arrives
    │
    ▼
CORS Middleware (before)
    │
    ├─ OPTIONS request? → Handle preflight → Return 200
    │
    └─ Other request? → Continue
        │
        ▼
Security Validation (via AppService)
    │
    ├─ JWKS Bearer Provider? → Validate JWT with JWKS
    │
    └─ SPIFFE Provider? → Validate SPIFFE SVID with JWKS
        │
        ▼
Handler Execution
    │
    ▼
CORS Middleware (after)
    │
    └─ Add CORS headers to response
```

---

## 7. Identified Issues and Recommendations

### 7.1 Issue: Test Isolation Breaking Interoperability

**Problem**: Fixing one test breaks another because tests don't account for system interactions.

**Root Cause**: Tests are written in isolation without considering:
- CORS is always present (even if permissive)
- SPIFFE requires JWKS (enforced)
- JWKS can be used independently

**Recommendation**: 
1. Create integration test suite for system interactions
2. Document test setup requirements (CORS always present, SPIFFE requires JWKS)
3. Add helper functions that set up complete system configurations

### 7.2 Issue: Missing Integration Tests

**Problem**: No tests verify CORS + JWKS + SPIFFE interoperability.

**Recommendation**: Add integration tests:
- `test_cors_with_jwks_bearer_provider()` - Verify CORS headers on JWKS-authenticated requests
- `test_cors_with_spiffe_provider()` - Verify CORS headers on SPIFFE-authenticated requests
- `test_jwks_independent_usage()` - Verify JWKS works without SPIFFE
- `test_spiffe_requires_jwks()` - Verify SPIFFE fails without JWKS (security requirement)

### 7.3 Issue: Configuration Validation

**Problem**: No validation that SPIFFE providers have JWKS configured at startup.

**Current State**: Validation happens at runtime (fail-secure, but late).

**Recommendation**: 
- Add startup validation in `SpiffeProvider::jwks_url()` to warn if not set
- Consider making `jwks_url()` return `Result` instead of `Self` to force configuration
- Or add a `build()` method that validates required fields

### 7.4 Issue: Documentation Clarity

**Problem**: Documentation doesn't clearly state the dependency relationships.

**Recommendation**: 
- Update `src/security/mod.rs` to clearly document:
  - JWKS can be used independently
  - SPIFFE requires JWKS
  - CORS is always required (even if relaxed)
- Add architecture diagram showing dependencies
- Document test setup requirements

---

## 8. Test Setup Requirements

### 8.1 For JWKS-Only Tests

**Required**:
- `JwksBearerProvider` with JWKS URL
- Mock JWKS server (for tests)
- Signed JWT tokens

**Not Required**:
- SPIFFE provider
- CORS middleware (but should be present for realistic testing)

### 8.2 For SPIFFE Tests

**Required**:
- `SpiffeProvider` with:
  - Trust domains configured
  - Audiences configured
  - **JWKS URL configured** (mandatory)
- Mock JWKS server (for tests)
- Signed SPIFFE JWT SVIDs

**Not Required**:
- CORS middleware (but should be present for realistic testing)

### 8.3 For Integration Tests

**Required**:
- CORS middleware (always)
- Security provider (JWKS or SPIFFE)
- Mock JWKS server (if using JWKS/SPIFFE)
- Signed tokens

---

## 9. Recommended Test Structure

### 9.1 Unit Tests (Current)

**Location**: `tests/spiffe_tests.rs`, `tests/security_tests.rs`

**Purpose**: Test individual provider functionality

**Setup**: Minimal - just the provider being tested

### 9.2 Integration Tests (Missing)

**Location**: `tests/integration_cors_jwks_spiffe_tests.rs` (new file)

**Purpose**: Test system interactions

**Test Cases**:
1. CORS + JWKS Bearer Provider
   - Preflight request with JWKS-authenticated endpoint
   - CORS headers on JWKS-authenticated response
   - Invalid origin rejection before JWKS validation

2. CORS + SPIFFE Provider
   - Preflight request with SPIFFE-authenticated endpoint
   - CORS headers on SPIFFE-authenticated response
   - Invalid origin rejection before SPIFFE validation

3. JWKS Independence
   - JWKS provider works without SPIFFE
   - No SPIFFE code in JWKS provider
   - JWKS can be used for non-SPIFFE JWT validation

4. SPIFFE JWKS Requirement
   - SPIFFE validation fails without JWKS URL
   - SPIFFE validation succeeds with JWKS URL
   - Algorithm mismatch validation works correctly

---

## 10. Configuration Validation Recommendations

### 10.1 Startup Validation

**Current**: Runtime validation (fail-secure)

**Recommended**: Startup validation with clear error messages

```rust
impl SpiffeProvider {
    pub fn build(self) -> Result<Self, SpiffeConfigError> {
        if self.jwks_url.is_none() {
            return Err(SpiffeConfigError::JwksUrlRequired);
        }
        if self.trust_domains.is_empty() {
            return Err(SpiffeConfigError::TrustDomainsRequired);
        }
        if self.audiences.is_empty() {
            return Err(SpiffeConfigError::AudiencesRequired);
        }
        Ok(self)
    }
}
```

### 10.2 Configuration Documentation

**Recommended**: Add to `src/security/spiffe/mod.rs`:

```rust
/// # Configuration Requirements
///
/// **MANDATORY**:
/// - `trust_domains()` - At least one trust domain must be configured
/// - `audiences()` - At least one audience must be configured
/// - `jwks_url()` - JWKS URL is **REQUIRED** for signature verification
///
/// **OPTIONAL**:
/// - `leeway()` - Clock skew tolerance (default: 60 seconds)
/// - `cookie_name()` - Cookie name for token extraction
/// - `revocation_checker()` - Token revocation checker
```

---

## 11. Code Quality Issues

### 11.1 Test Helper Functions

**Current**: Multiple helper functions with inconsistent patterns

**Issues**:
- `make_spiffe_jwt()` creates unsigned tokens (should be deprecated)
- `make_signed_spiffe_jwt_for_test()` uses hardcoded secret
- `create_provider_with_mock_jwks()` is good pattern but not used consistently

**Recommendation**: 
- Standardize on `create_provider_with_mock_jwks()` for all SPIFFE tests
- Deprecate `make_spiffe_jwt()` (or rename to `make_unsigned_spiffe_jwt_for_failure_tests()`)
- Document which helpers to use for which scenarios

### 11.2 Error Messages

**Current**: Some error messages don't clearly indicate the root cause

**Recommendation**: Improve error messages:
- "JWKS URL not configured" → "SPIFFE validation requires JWKS URL for signature verification. Configure with `.jwks_url()`"
- "Algorithm mismatch" → "Token algorithm (HS384) does not match JWKS key algorithm (HS256) for key 'k1'"

---

## 12. Security Considerations

### 12.1 Algorithm Mismatch Protection

**Status**: ✅ Implemented

**Implementation**: 
- SPIFFE stores algorithm with each key in cache
- Validates token algorithm matches JWKS key algorithm before signature verification
- Prevents algorithm confusion attacks

**Test Coverage**: ✅ `test_spiffe_jwks_algorithm_mismatch`

### 12.2 Signature Verification Requirement

**Status**: ✅ Enforced

**Implementation**:
- SPIFFE validation fails if `jwks_url` is `None`
- Fail-secure behavior (reject if not configured)

**Test Coverage**: ✅ Multiple tests verify this behavior

### 12.3 CORS Security

**Status**: ✅ Well-implemented

**Implementation**:
- Origin validation against whitelist
- Credentials validation (wildcard + credentials = panic)
- Route-specific CORS policies

**Test Coverage**: ✅ Good coverage

---

## 13. Performance Considerations

### 13.1 JWKS Caching

**Status**: ✅ Well-optimized

**Implementation**:
- In-memory cache with TTL
- Background refresh thread
- Sub-second TTL support
- Claims cache with LRU eviction

### 13.2 SPIFFE Caching

**Status**: ✅ Uses same caching strategy as JWKS

**Implementation**:
- Same cache structure as JWKS provider
- Algorithm stored with key for validation

### 13.3 CORS Processing

**Status**: ✅ Startup-time configuration

**Implementation**:
- All CORS configuration processed at startup
- No runtime allocations in hot path
- Route-specific policies pre-computed

---

## 14. Recommendations Summary

### 14.1 Immediate Actions

1. **Create Integration Test Suite**
   - File: `tests/integration_cors_jwks_spiffe_tests.rs`
   - Test all system interactions
   - Verify interoperability

2. **Standardize Test Helpers**
   - Use `create_provider_with_mock_jwks()` consistently
   - Deprecate `make_spiffe_jwt()` for success cases
   - Document helper function usage

3. **Improve Error Messages**
   - Clear messages for missing JWKS URL
   - Clear messages for algorithm mismatches
   - Configuration validation errors

### 14.2 Short-term Improvements

1. **Startup Validation**
   - Add `build()` method to `SpiffeProvider` for validation
   - Validate all required fields at startup
   - Clear error messages for missing configuration

2. **Documentation Updates**
   - Update `src/security/mod.rs` with dependency diagram
   - Document test setup requirements
   - Add architecture overview

3. **Test Coverage**
   - Add integration tests for system interactions
   - Add tests for configuration validation
   - Add tests for error scenarios

### 14.3 Long-term Improvements

1. **Configuration Builder Pattern**
   - Consider builder pattern with `build()` validation
   - Type-safe configuration
   - Compile-time guarantees where possible

2. **Monitoring and Observability**
   - Add metrics for JWKS refresh failures
   - Add metrics for algorithm mismatches
   - Add metrics for CORS rejections

---

## 15. Test Fixes Applied

### 15.1 Fixed Tests

All SPIFFE tests have been updated to:
- Use signed tokens with `make_signed_spiffe_jwt_for_test()`
- Configure JWKS URL with `create_provider_with_mock_jwks()`
- Wait for JWKS fetch with `thread::sleep(Duration::from_millis(200))`

**Tests Fixed**:
- `test_spiffe_authorization_header_case_insensitive`
- `test_spiffe_bearer_token_whitespace_handling`
- `test_spiffe_bearer_prefix_case_sensitivity`
- `test_spiffe_exp_boundary_exact`
- `test_spiffe_exp_boundary_leeway`
- `test_spiffe_extract_token_cookie_fallback`
- `test_spiffe_extract_token_from_cookie`
- `test_spiffe_extract_token_prefers_header_over_cookie`
- `test_spiffe_id_path_special_characters`
- `test_spiffe_id_very_long_path`
- `test_spiffe_leeway_configuration`
- `test_spiffe_jwks_algorithm_mismatch`
- `test_spiffe_validation_valid_svid`
- `test_spiffe_validation_audience_string`
- `test_spiffe_validation_audience_array`
- `test_spiffe_multiple_trust_domains`
- `test_spiffe_multiple_authorization_headers`
- `test_spiffe_refresh_jwks_if_needed_early_return`
- `test_spiffe_refresh_jwks_if_needed_no_jwks_configured`
- `test_spiffe_token_revocation`
- `test_spiffe_token_revocation_no_jti`
- `test_spiffe_trust_domain_case_sensitivity`
- `test_spiffe_trust_domain_exact_match_required`
- `test_spiffe_leeway_configuration_edge_cases`

### 15.2 New Edge Case Tests Added

- `test_spiffe_audience_as_object`
- `test_spiffe_audience_as_number`
- `test_spiffe_audience_as_boolean`
- `test_spiffe_audience_as_null`
- `test_spiffe_audience_missing_with_valid_signature`
- `test_spiffe_signature_verification_with_disabled_audience_validation`

---

## 16. Conclusion

The three systems (CORS, JWKS, SPIFFE) are well-architected with clear separation of concerns:

1. **CORS**: Always required, operates independently
2. **JWKS**: Can be used independently, no SPIFFE dependency
3. **SPIFFE**: Requires JWKS, properly enforced

**Key Strengths**:
- Clear dependency relationships
- Proper security enforcement
- Good test coverage (after fixes)
- Algorithm mismatch protection

**Areas for Improvement**:
- Integration test coverage
- Test helper standardization
- Startup configuration validation
- Documentation clarity

**Next Steps**:
1. Create integration test suite
2. Standardize test helpers
3. Add startup validation
4. Update documentation

---

## Appendix A: File Structure

```
src/
├── middleware/
│   ├── cors/
│   │   ├── mod.rs          # Core CORS middleware
│   │   ├── builder.rs      # Builder pattern
│   │   ├── route_config.rs # Route-specific policies
│   │   └── error.rs        # CORS errors
│   └── mod.rs              # Middleware exports
│
├── security/
│   ├── jwks_bearer/
│   │   ├── mod.rs          # JWKS provider (independent)
│   │   └── validation.rs   # JWT validation
│   ├── spiffe/
│   │   ├── mod.rs          # SPIFFE provider (requires JWKS)
│   │   ├── validation.rs   # SPIFFE validation
│   │   └── revocation.rs   # Token revocation
│   └── mod.rs              # Security exports
│
tests/
├── auth_cors_tests.rs       # CORS tests
├── security_tests.rs        # JWKS tests
├── spiffe_tests.rs          # SPIFFE tests
└── middleware_tests.rs      # Middleware tests
```

---

## Appendix B: Configuration Examples

### B.1 JWKS-Only Configuration

```rust
// JWKS Bearer Provider (no SPIFFE)
let jwks_provider = JwksBearerProvider::new("https://auth.example.com/.well-known/jwks.json")
    .issuer("https://auth.example.com")
    .audience("my-api")
    .leeway(60);

service.register_security_provider("bearerAuth", Arc::new(jwks_provider));
```

### B.2 SPIFFE Configuration

```rust
// SPIFFE Provider (requires JWKS)
let spiffe_provider = SpiffeProvider::new()
    .trust_domains(&["example.com"])
    .audiences(&["api.example.com"])
    .jwks_url("https://spiffe.example.com/.well-known/jwks.json") // REQUIRED
    .leeway(60);

service.register_security_provider("spiffeAuth", Arc::new(spiffe_provider));
```

### B.3 CORS Configuration

```yaml
# config.yaml
cors:
  origins:
    - "https://example.com"
  allowed_headers:
    - "Content-Type"
    - "Authorization"
  allowed_methods:
    - "GET"
    - "POST"
    - "PUT"
    - "DELETE"
    - "OPTIONS"
  allow_credentials: false
```

---

## Appendix C: Test Helper Functions

### C.1 Recommended Helpers

**For SPIFFE Tests**:
- `create_provider_with_mock_jwks()` - Sets up provider with mock JWKS
- `make_signed_spiffe_jwt_for_test()` - Creates signed token with test secret
- `make_signed_spiffe_jwt_array_aud_for_test()` - Creates signed token with array audience

**For JWKS Tests**:
- Use existing JWKS test helpers in `tests/security_tests.rs`

**For CORS Tests**:
- Use existing CORS test helpers in `tests/auth_cors_tests.rs`

### C.2 Deprecated Helpers

- `make_spiffe_jwt()` - Creates unsigned token (only for failure tests)
- `make_spiffe_jwt_array_aud()` - Creates unsigned token with array audience (only for failure tests)

---

*Audit Date: 2024-01-XX*
*Auditor: AI Assistant*
*Status: Complete*
