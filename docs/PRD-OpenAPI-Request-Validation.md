# PRD: OpenAPI Request Validation & Response Enforcement

**Product Requirements Document**  
**Version:** 1.0  
**Date:** January 2025  
**Project:** BRRTRouter  
**Status:** Draft  

## Executive Summary

This PRD outlines the implementation of comprehensive OpenAPI request validation and response enforcement in BRRTRouter to ensure that only requests conforming to OpenAPI specifications reach backend handlers, providing robust API contract enforcement and backend protection.

## Problem Statement

### Current State
BRRTRouter currently implements **70% OpenAPI compliance** for request validation:
- ✅ JSON schema validation for request/response bodies
- ✅ Security validation (authentication/authorization)
- ✅ Route matching and path parameter extraction
- ✅ Parameter style support (Form, SpaceDelimited, etc.)

### Critical Gaps
- ❌ **Parameter validation against OpenAPI specs** - Parameters are extracted but not validated
- ❌ **Required parameter enforcement** - Missing required parameters are not rejected
- ❌ **Parameter type/constraint validation** - No validation of min/max, pattern, enum constraints
- ❌ **Header parameter validation** - Headers extracted but not validated
- ❌ **Content-Type validation** - No validation of request media types
- ❌ **Request size validation** - No enforcement of payload size limits

### Business Impact
- **Security Risk**: Invalid requests can reach handlers, potentially causing errors or security issues
- **API Contract Violation**: APIs don't enforce their published contracts
- **Developer Experience**: Poor error messages for malformed requests
- **Backend Protection**: Handlers must implement their own validation logic

## Solution Overview

Implement a comprehensive **OpenAPI Request Validation Layer** that intercepts all requests before they reach handlers and validates them against the complete OpenAPI specification.

### Key Principles
1. **Fail Fast**: Reject non-conforming requests before handler execution
2. **Comprehensive Coverage**: Validate all aspects of OpenAPI parameter definitions
3. **Clear Error Messages**: Provide detailed validation error responses
4. **Performance**: Minimal overhead for valid requests
5. **Backward Compatibility**: No breaking changes to existing handlers

## Requirements

### 1. Core Validation Features

#### 1.1 Parameter Validation
**Priority: P0 (Critical)**

**Requirements:**
- Validate all parameter types: path, query, header, cookie
- Enforce required parameter presence
- Validate parameter types against JSON schemas
- Support all OpenAPI parameter styles (form, spaceDelimited, pipeDelimited, etc.)
- Validate parameter constraints (min/max length, pattern, enum values)

**Acceptance Criteria:**
- [ ] Required parameters missing → 400 Bad Request
- [ ] Invalid parameter types → 400 Bad Request with type error details
- [ ] Parameter constraint violations → 400 Bad Request with constraint details
- [ ] All parameter styles correctly parsed and validated
- [ ] Case-insensitive header parameter validation

#### 1.2 Content-Type Validation
**Priority: P0 (Critical)**

**Requirements:**
- Validate request Content-Type against OpenAPI requestBody definitions
- Support multiple content types per operation
- Reject unsupported media types

**Acceptance Criteria:**
- [ ] Unsupported Content-Type → 415 Unsupported Media Type
- [ ] Missing Content-Type for operations requiring body → 400 Bad Request
- [ ] Multiple supported content types handled correctly

#### 1.3 Request Size Validation
**Priority: P1 (High)**

**Requirements:**
- Validate request body size against OpenAPI limits
- Configurable global size limits
- Per-operation size limits from OpenAPI spec

**Acceptance Criteria:**
- [ ] Oversized requests → 413 Payload Too Large
- [ ] Configurable size limits respected
- [ ] Efficient size checking without full body parsing

#### 1.4 Enhanced JSON Schema Validation
**Priority: P1 (High)**

**Requirements:**
- Extend existing JSON schema validation
- Support nested schema references
- Validate additional JSON Schema constraints (format, pattern, etc.)

**Acceptance Criteria:**
- [ ] All JSON Schema constraints enforced
- [ ] Nested $ref resolution works correctly
- [ ] Format validation (email, uri, date-time, etc.)

### 2. Response Validation Enhancement

#### 2.1 Strict Response Schema Validation
**Priority: P1 (High)**

**Requirements:**
- Validate all response bodies against OpenAPI schemas
- Support multiple response content types
- Validate response headers against OpenAPI definitions

**Acceptance Criteria:**
- [ ] Invalid response schemas → 500 Internal Server Error
- [ ] Response header validation
- [ ] Multiple response content types supported
- [ ] Development mode vs production mode behavior

#### 2.2 Response Status Code Validation
**Priority: P2 (Medium)**

**Requirements:**
- Validate response status codes against OpenAPI definitions
- Support default response handling
- Configurable strict mode for undefined status codes

**Acceptance Criteria:**
- [ ] Undefined status codes handled appropriately
- [ ] Default response schemas applied correctly
- [ ] Strict mode configuration option

### 3. Error Handling & Developer Experience

#### 3.1 Detailed Error Messages
**Priority: P0 (Critical)**

**Requirements:**
- Structured error responses with validation details
- Clear indication of which parameter/field failed validation
- Suggestions for fixing validation errors
- Consistent error response format

**Acceptance Criteria:**
- [ ] Error responses follow RFC 7807 Problem Details format
- [ ] Specific field/parameter identified in errors
- [ ] Human-readable error messages
- [ ] Machine-readable error codes

#### 3.2 Development Mode Features
**Priority: P1 (High)**

**Requirements:**
- Verbose validation error logging
- Request/response validation debugging
- Performance metrics for validation overhead
- Validation bypass for development/testing

**Acceptance Criteria:**
- [ ] Configurable validation logging levels
- [ ] Performance impact measurement
- [ ] Development-only validation bypass
- [ ] Validation statistics endpoint

### 4. Performance & Scalability

#### 4.1 Validation Performance
**Priority: P1 (High)**

**Requirements:**
- Minimal performance impact on valid requests
- Efficient parameter parsing and validation
- Schema compilation caching
- Parallel validation where possible

**Acceptance Criteria:**
- [ ] <1ms validation overhead for typical requests
- [ ] Schema compilation cached and reused
- [ ] Memory usage remains bounded
- [ ] Validation scales with request volume

#### 4.2 Configuration & Flexibility
**Priority: P2 (Medium)**

**Requirements:**
- Configurable validation strictness levels
- Per-operation validation overrides
- Runtime validation configuration updates
- Validation rule customization

**Acceptance Criteria:**
- [ ] Validation strictness configurable (strict/lenient/off)
- [ ] Per-operation validation configuration
- [ ] Hot-reload of validation configuration
- [ ] Custom validation rule plugins

## Technical Implementation

### 1. Architecture Overview

```
Request → Router → [NEW] Validation Layer → Security → Handler
                      ↓
                 Parameter Validation
                 Content-Type Validation  
                 Schema Validation
                 Size Validation
```

### 2. Core Components

#### 2.1 Request Validator (`src/validator/request.rs`)
```rust
pub struct RequestValidator {
    schema_cache: Arc<RwLock<HashMap<String, CompiledSchema>>>,
    config: ValidationConfig,
}

impl RequestValidator {
    pub fn validate_request(&self, route: &RouteMeta, request: &ParsedRequest) -> ValidationResult;
    pub fn validate_parameters(&self, params: &[ParameterMeta], request: &ParsedRequest) -> ValidationResult;
    pub fn validate_content_type(&self, route: &RouteMeta, content_type: Option<&str>) -> ValidationResult;
    pub fn validate_request_size(&self, route: &RouteMeta, body_size: usize) -> ValidationResult;
}
```

#### 2.2 Response Validator (`src/validator/response.rs`)
```rust
pub struct ResponseValidator {
    schema_cache: Arc<RwLock<HashMap<String, CompiledSchema>>>,
    config: ValidationConfig,
}

impl ResponseValidator {
    pub fn validate_response(&self, route: &RouteMeta, response: &HandlerResponse) -> ValidationResult;
    pub fn validate_response_headers(&self, route: &RouteMeta, headers: &HashMap<String, String>) -> ValidationResult;
}
```

#### 2.3 Validation Configuration (`src/validator/config.rs`)
```rust
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub strict_mode: bool,
    pub max_request_size: usize,
    pub validate_responses: bool,
    pub development_mode: bool,
    pub parameter_validation: ParameterValidationConfig,
}
```

### 3. Integration Points

#### 3.1 AppService Integration
Update `src/server/service.rs` to include validation layer:

```rust
impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        // ... existing code ...
        
        if let Some(mut route_match) = route_opt {
            // NEW: Comprehensive request validation
            if let Err(validation_error) = self.request_validator.validate_request(&route_match.route, &parsed_request) {
                return write_validation_error(res, validation_error);
            }
            
            // ... existing security validation ...
            
            // ... handler dispatch ...
            
            // NEW: Response validation
            if let Some(hr) = handler_response {
                if let Err(validation_error) = self.response_validator.validate_response(&route_match.route, &hr) {
                    return write_validation_error(res, validation_error);
                }
            }
        }
    }
}
```

#### 3.2 RouteMeta Enhancement
Extend `src/spec/types.rs` to include validation metadata:

```rust
#[derive(Debug, Clone)]
pub struct RouteMeta {
    // ... existing fields ...
    pub request_content_types: Vec<String>,
    pub max_request_size: Option<usize>,
    pub response_headers: HashMap<u16, Vec<HeaderMeta>>,
    pub strict_validation: bool,
}
```

### 4. Error Response Format

Follow RFC 7807 Problem Details format:

```json
{
  "type": "https://brrtrouter.dev/problems/validation-error",
  "title": "Request Validation Failed",
  "status": 400,
  "detail": "Parameter 'age' must be a positive integer",
  "instance": "/pets/123",
  "validation_errors": [
    {
      "field": "age",
      "location": "query",
      "message": "Value '-5' is less than minimum 0",
      "constraint": "minimum",
      "value": "-5"
    }
  ]
}
```

## Testing Strategy

### 1. Unit Tests
- Parameter validation logic
- Content-Type validation
- Schema compilation and caching
- Error message formatting

### 2. Integration Tests
- End-to-end request validation
- Multiple parameter types and styles
- Error response format validation
- Performance benchmarks

### 3. Test Coverage Requirements
- **Target**: 90% code coverage for validation modules
- **Critical paths**: 100% coverage for validation logic
- **Performance tests**: Validation overhead < 1ms

## Implementation Plan

### Phase 1: Core Parameter Validation (4 weeks)
- [ ] Implement RequestValidator with parameter validation
- [ ] Add required parameter checking
- [ ] Implement parameter type validation
- [ ] Add parameter constraint validation
- [ ] Integrate with AppService

### Phase 2: Content-Type & Size Validation (2 weeks)
- [ ] Implement Content-Type validation
- [ ] Add request size validation
- [ ] Enhance error message formatting
- [ ] Add development mode features

### Phase 3: Response Validation Enhancement (3 weeks)
- [ ] Implement ResponseValidator
- [ ] Add response header validation
- [ ] Implement response status code validation
- [ ] Add response schema validation

### Phase 4: Performance & Configuration (2 weeks)
- [ ] Implement schema caching
- [ ] Add validation configuration
- [ ] Performance optimization
- [ ] Add validation metrics

### Phase 5: Testing & Documentation (1 week)
- [ ] Comprehensive test suite
- [ ] Performance benchmarks
- [ ] Documentation updates
- [ ] Migration guide

## Success Metrics

### 1. Functional Metrics
- **100% OpenAPI compliance** for request validation
- **Zero invalid requests** reach handlers
- **<1ms validation overhead** for typical requests
- **90% test coverage** for validation modules

### 2. Quality Metrics
- **Clear error messages** for all validation failures
- **Consistent error format** across all validation types
- **Backward compatibility** maintained
- **Performance regression** < 5% for valid requests

### 3. Developer Experience Metrics
- **Reduced handler validation code** by 80%
- **Improved error debugging** time
- **Faster development cycles** with clear validation errors

## Risk Assessment

### High Risk
- **Performance impact**: Validation overhead could affect high-throughput scenarios
  - *Mitigation*: Comprehensive benchmarking and optimization
- **Breaking changes**: Strict validation might break existing clients
  - *Mitigation*: Configurable validation levels and gradual rollout

### Medium Risk
- **Complex parameter styles**: Edge cases in parameter parsing
  - *Mitigation*: Comprehensive test suite covering all OpenAPI parameter styles
- **Memory usage**: Schema caching might increase memory footprint
  - *Mitigation*: Bounded cache with LRU eviction

### Low Risk
- **Configuration complexity**: Too many validation options
  - *Mitigation*: Sensible defaults and clear documentation

## Dependencies

### Internal Dependencies
- Enhanced `ParameterMeta` structure
- Updated `RouteMeta` with validation metadata
- Extended error response handling

### External Dependencies
- `jsonschema` crate (already included)
- Potential new crates for advanced validation features
- Performance profiling tools

## Conclusion

This PRD outlines a comprehensive approach to implementing strict OpenAPI request validation in BRRTRouter. The implementation will transform BRRTRouter from 70% to 100% OpenAPI compliance, providing robust backend protection while maintaining excellent developer experience and performance.

The phased approach ensures incremental delivery of value while managing complexity and risk. The focus on performance, clear error messages, and developer experience will make BRRTRouter a best-in-class OpenAPI-native router. 