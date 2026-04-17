# SPIFFE Support for Enterprise Windows Single Sign-On

## Overview

This document outlines the implementation plan for adding SPIFFE (Secure Production Identity Framework for Everyone) support to BRRTRouter, specifically targeting enterprise Windows environments with single sign-on (SSO) capabilities.

## SPIFFE Background

SPIFFE provides a framework for securely identifying and authenticating services in dynamic environments. Key concepts:

- **SPIFFE ID**: Unique identifier in format `spiffe://trust-domain/path`
- **SVID (SPIFFE Verifiable Identity Document)**: Cryptographically verifiable identity document (JWT or X.509)
- **Trust Domain**: The trust root for a set of workloads
- **Workload API**: Interface for fetching SVIDs (optional, for dynamic workloads)

## Implementation Goals

1. **JWT SVID Validation**: Validate SPIFFE JWT SVIDs with proper claim verification
2. **SPIFFE ID Extraction**: Extract and validate SPIFFE IDs from JWT claims
3. **Windows Integration**: Support Windows workload attestation and Active Directory integration
4. **OpenAPI Integration**: Register SPIFFE provider via OpenAPI security schemes
5. **Enterprise SSO**: Enable seamless SSO for Windows enterprise users

## Architecture

### Phase 1: Core SPIFFE Provider (P0)

**Goal**: Basic JWT SVID validation with SPIFFE ID extraction

**Components**:
- `SpiffeProvider` struct implementing `SecurityProvider`
- SPIFFE ID validation (format: `spiffe://trust-domain/path`)
- JWT SVID claim validation:
  - `sub` claim must be a valid SPIFFE ID
  - `aud` (audience) validation
  - `exp` (expiration) validation
  - `iss` (issuer) validation (trust domain)
- Integration with existing JWT validation infrastructure

**Files to Create**:
- `src/security/spiffe/mod.rs` - Main provider implementation
- `src/security/spiffe/validation.rs` - SPIFFE-specific validation logic
- `src/security/spiffe/claims.rs` - SPIFFE claim structures

**Dependencies**:
- Reuse `jsonwebtoken` from `jwks_bearer`
- Reuse `serde_json` for claim parsing
- Add `regex` for SPIFFE ID format validation (if not already present)

### Phase 2: Trust Domain Configuration (P1)

**Goal**: Configurable trust domains and audience validation

**Components**:
- Trust domain whitelist configuration
- Audience validation against configured audiences
- Trust domain extraction from SPIFFE ID
- Multi-trust-domain support

**Configuration**:
```yaml
# config.yaml
security:
  spiffe:
    trust_domains:
      - "example.com"
      - "enterprise.local"
    audiences:
      - "api.example.com"
      - "brrtrouter"
    leeway: 60  # seconds
```

### Phase 3: Windows Workload Attestation (P2)

**Goal**: Windows-specific workload attestation support

**Components**:
- Windows workload attestation node API integration (optional)
- Active Directory (AD) integration for user mapping
- Windows service principal name (SPN) validation
- Kerberos token support (future)

**Windows-Specific Features**:
- Map SPIFFE IDs to Windows user accounts
- Validate Windows service identities
- Support for Windows container workloads
- Integration with Windows Certificate Store

### Phase 4: Workload API Integration (P3)

**Goal**: Dynamic SVID fetching from SPIFFE Workload API

**Components**:
- Unix domain socket client for Workload API
- Named pipe client for Windows Workload API
- SVID caching and refresh logic
- Background refresh thread (similar to JWKS)

**Note**: This is optional for most use cases where SVIDs are passed as Bearer tokens.

## Implementation Details

### SPIFFE ID Format

SPIFFE IDs follow the format: `spiffe://trust-domain/path`

Examples:
- `spiffe://example.com/api/users`
- `spiffe://enterprise.local/windows/service/api`
- `spiffe://prod.example.com/frontend/web`

### JWT SVID Claims

SPIFFE JWT SVIDs contain standard JWT claims plus SPIFFE-specific claims:

- `sub` (subject): **Required** - Must be a valid SPIFFE ID
- `aud` (audience): **Required** - Must match configured audiences
- `exp` (expiration): **Required** - Standard JWT expiration
- `iat` (issued at): **Required** - Standard JWT issued time
- `iss` (issuer): **Optional** - Trust domain (extracted from `sub` if not present)
- `nbf` (not before): **Optional** - Standard JWT not-before time

### Validation Flow

1. Extract JWT token from `Authorization: Bearer {token}` header
2. Parse JWT header to get algorithm and key ID (if present)
3. Validate JWT signature (using JWKS or configured public key)
4. Extract and validate `sub` claim (must be valid SPIFFE ID format)
5. Extract trust domain from SPIFFE ID
6. Validate trust domain against whitelist
7. Validate `aud` claim against configured audiences
8. Validate `exp` claim (with leeway)
9. Extract SPIFFE ID and make available to handlers

### Windows Integration Strategy

For Windows enterprise SSO, we'll support two approaches:

1. **Direct SVID Validation**: Validate SPIFFE JWT SVIDs passed as Bearer tokens
   - Works with any SPIFFE-compatible identity provider
   - No Windows-specific dependencies

2. **Windows Workload Attestation** (Phase 3):
   - Integrate with Windows Workload API (named pipes)
   - Map SPIFFE IDs to Windows user accounts
   - Support Windows container workloads

## OpenAPI Integration

SPIFFE provider will be registered via OpenAPI security schemes:

```yaml
components:
  securitySchemes:
    spiffeAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
      description: SPIFFE JWT SVID authentication
      x-brrtrouter-spiffe:
        trust_domains:
          - "example.com"
        audiences:
          - "api.example.com"
```

## Testing Strategy

1. **Unit Tests**:
   - SPIFFE ID format validation
   - JWT SVID claim validation
   - Trust domain validation
   - Audience validation

2. **Integration Tests**:
   - Full request validation flow
   - Multiple trust domains
   - Invalid SVID rejection
   - Expired SVID handling

3. **Windows-Specific Tests** (Phase 3):
   - Windows workload attestation
   - AD user mapping
   - Windows container support

## Security Considerations

1. **Trust Domain Validation**: Only accept SVIDs from configured trust domains
2. **Audience Validation**: Strict audience checking prevents token reuse
3. **Signature Verification**: All SVIDs must be cryptographically verified
4. **Expiration Checking**: Enforce token expiration with configurable leeway
5. **SPIFFE ID Format**: Strict format validation prevents injection attacks

## Dependencies

- `jsonwebtoken` - JWT parsing and validation (already in use)
- `serde_json` - JSON parsing (already in use)
- `regex` - SPIFFE ID format validation (already in use for CORS)
- `url` - SPIFFE ID parsing (may need to add)

## Migration Path

1. **Phase 1**: Core provider (no breaking changes)
2. **Phase 2**: Configuration support (adds config.yaml options)
3. **Phase 3**: Windows-specific features (optional, opt-in)
4. **Phase 4**: Workload API (optional, advanced use case)

## Success Criteria

- ✅ JWT SVID validation with SPIFFE ID extraction
- ✅ Trust domain whitelist support
- ✅ Audience validation
- ✅ OpenAPI integration
- ✅ Comprehensive test coverage
- ✅ Windows workload attestation (Phase 3)
- ✅ Documentation and examples

## Next Steps

1. Create `SpiffeProvider` struct and basic validation
2. Implement SPIFFE ID format validation
3. Integrate with existing JWT validation infrastructure
4. Add OpenAPI extension support for SPIFFE configuration
5. Write comprehensive tests
6. Document usage and examples

