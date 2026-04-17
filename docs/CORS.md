# CORS (Cross-Origin Resource Sharing) Configuration

BRRTRouter provides comprehensive CORS support with route-specific configuration via OpenAPI extensions and global configuration via `config.yaml`.

## Overview

CORS middleware handles:
- **Preflight requests** (OPTIONS) with proper validation
- **Origin validation** against allowed origins whitelist
- **Route-specific overrides** via OpenAPI `x-cors` extension
- **Credentials support** with proper security validation
- **Exposed headers** for JavaScript access
- **Preflight caching** to reduce overhead

## Global Configuration (config.yaml)

CORS is configured globally in `config.yaml`:

```yaml
cors:
  # Origins are environment-specific and should be in config.yaml
  origins:
    - "https://example.com"
    - "https://api.example.com"
    # Use "*" to allow all origins (insecure, not recommended for production)
    # - "*"
  
  # Global CORS settings (can be overridden per-route via OpenAPI x-cors)
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
  expose_headers: []
  max_age: null  # Preflight cache duration in seconds (null = no caching)
```

### Configuration Options

- **`origins`**: List of allowed origins (required for cross-origin requests)
  - Use `"*"` to allow all origins (insecure, not recommended for production)
  - **Cannot use `"*"` with `allow_credentials: true`** (CORS spec violation)
- **`allowed_headers`**: Headers that clients can send in requests
- **`allowed_methods`**: HTTP methods allowed for cross-origin requests
- **`allow_credentials`**: If `true`, sets `Access-Control-Allow-Credentials: true`
  - **Cannot be used with wildcard origin (`*`)**
- **`expose_headers`**: Headers exposed to JavaScript (e.g., `["X-Total-Count"]`)
- **`max_age`**: Preflight cache duration in seconds (e.g., `3600` for 1 hour)

## Route-Specific Configuration (OpenAPI x-cors)

Routes can override global CORS settings using the `x-cors` extension in OpenAPI specifications.

### Supported Formats

#### 1. Object Configuration (Custom Policy)

Override specific CORS settings for a route:

```yaml
paths:
  /api/pets:
    get:
      operationId: list_pets
      x-cors:
        allowCredentials: true
        allowedHeaders:
          - "Content-Type"
          - "Authorization"
          - "X-Custom-Header"
        allowedMethods:
          - "GET"
          - "POST"
        exposeHeaders:
          - "X-Total-Count"
          - "X-Page-Number"
        maxAge: 3600
      responses:
        "200":
          description: List of pets
```

**Note**: Origins are **NOT** specified in `x-cors` - they come from `config.yaml` (environment-specific). The `x-cors` extension can only override other CORS settings (methods, headers, credentials, etc.).

#### 2. String: "inherit" (Use Global Config)

Explicitly inherit global CORS configuration:

```yaml
paths:
  /api/public:
    get:
      operationId: get_public_data
      x-cors: "inherit"  # Uses global CORS config from config.yaml
      responses:
        "200":
          description: Public data
```

**Behavior**: Uses the global CORS configuration from `config.yaml`. This is the default behavior when `x-cors` is not present.

#### 3. Boolean: false (Disable CORS)

Disable CORS for a specific route (no CORS headers will be added):

```yaml
paths:
  /api/internal:
    post:
      operationId: internal_operation
      x-cors: false  # Disables CORS - no CORS headers will be added
      responses:
        "200":
          description: Internal operation result
```

**Behavior**: **No CORS headers are added** for this route, regardless of global CORS settings. This is useful for:
- Internal APIs that should not be accessible from browsers
- Sensitive endpoints that must not allow cross-origin access
- Routes that should only be accessed same-origin

**Security Note**: Setting `x-cors: false` ensures that even if global CORS is permissive, this route will not have CORS headers, preventing unintended cross-origin access.

### Policy Comparison

| Format | Policy | Behavior |
|--------|--------|----------|
| Not present | `Inherit` | Uses global CORS config from `config.yaml` |
| `x-cors: "inherit"` | `Inherit` | Uses global CORS config from `config.yaml` |
| `x-cors: false` | `Disabled` | **No CORS headers** - CORS is disabled for this route |
| `x-cors: { ... }` | `Custom` | Uses route-specific CORS config (merged with global origins) |

## Security Considerations

### Wildcard Origins with Credentials

**Invalid Configuration** (will panic at startup):
```yaml
cors:
  origins:
    - "*"
  allow_credentials: true  # ❌ Cannot use wildcard with credentials
```

**Valid Configuration**:
```yaml
cors:
  origins:
    - "https://example.com"
    - "https://app.example.com"
  allow_credentials: true  # ✅ Specific origins with credentials
```

### Disabling CORS for Sensitive Endpoints

For sensitive endpoints that should never allow cross-origin access:

```yaml
paths:
  /api/admin/users:
    post:
      operationId: create_user
      x-cors: false  # Disables CORS - prevents cross-origin access
      security:
        - ApiKeyAuth: []
      responses:
        "201":
          description: User created
```

This ensures that even if global CORS is configured, this endpoint will not have CORS headers, preventing cross-origin requests from browsers.

## Examples

### Example 1: Public API with Credentials

```yaml
# config.yaml
cors:
  origins:
    - "https://app.example.com"
  allow_credentials: true
  allowed_headers:
    - "Content-Type"
    - "Authorization"

# openapi.yaml
paths:
  /api/pets:
    get:
      x-cors:
        exposeHeaders:
          - "X-Total-Count"
        maxAge: 3600
```

Result: Uses global origins and credentials, but adds custom exposed headers and preflight caching.

### Example 2: Mixed Public and Internal Routes

```yaml
# config.yaml
cors:
  origins:
    - "https://app.example.com"
  allow_credentials: true

# openapi.yaml
paths:
  /api/public:
    get:
      x-cors: "inherit"  # Uses global config (allows cross-origin)
      
  /api/internal:
    post:
      x-cors: false  # Disables CORS (no cross-origin access)
```

Result: `/api/public` allows cross-origin requests, `/api/internal` does not.

### Example 3: Route-Specific Methods

```yaml
# config.yaml
cors:
  origins:
    - "https://app.example.com"
  allowed_methods:
    - "GET"
    - "POST"

# openapi.yaml
paths:
  /api/upload:
    post:
      x-cors:
        allowedMethods:
          - "POST"
          - "OPTIONS"
        maxAge: 7200
```

Result: Only POST and OPTIONS are allowed for this route, overriding global methods.

## Implementation Details

### Startup Processing

All CORS configuration is processed **at startup time**:
1. Global CORS config is loaded from `config.yaml`
2. Route-specific `x-cors` extensions are extracted from OpenAPI spec
3. Origins from `config.yaml` are merged into route-specific configs
4. All validation (wildcard + credentials) happens at startup
5. Resulting policies are stored in a HashMap for O(1) lookups

**JSF Compliance**: No runtime parsing or allocation - all processing happens once at startup.

### Request Processing

For each request:
1. **OPTIONS requests** (preflight):
   - Origin is validated against route-specific or global config
   - Requested method and headers are validated
   - Returns 200 with CORS headers if valid, 403 if invalid
   - If CORS is disabled (`x-cors: false`), returns 200 without CORS headers

2. **Non-OPTIONS requests**:
   - Origin is validated in `before()` middleware
   - If invalid, returns 403 immediately
   - If CORS is disabled (`x-cors: false`), proceeds without CORS validation
   - CORS headers are added in `after()` middleware if origin is valid
   - If CORS is disabled, no CORS headers are added

### Same-Origin Requests

Same-origin requests (matching Host header) skip CORS headers entirely - this is correct behavior per CORS specification.

## Testing

See `tests/auth_cors_tests.rs` and `tests/middleware_tests.rs` for comprehensive CORS test coverage.

Key test scenarios:
- Global CORS configuration
- Route-specific CORS overrides
- `x-cors: false` (disabled CORS)
- `x-cors: "inherit"` (explicit inheritance)
- Wildcard origins
- Credentials with specific origins
- Invalid configurations (wildcard + credentials)

## Troubleshooting

### CORS headers not appearing

1. Check that CORS middleware is registered in your service
2. Verify that the request has an `Origin` header (CORS only applies to cross-origin requests)
3. Check if the route has `x-cors: false` (disables CORS)
4. Verify origin is in the allowed origins list

### Preflight requests returning 403

1. Check that the requested method is in `allowed_methods`
2. Verify that requested headers are in `allowed_headers`
3. Ensure the origin is valid (not blocked by route-specific config)

### Startup panic: "Cannot use wildcard origin (*) with credentials"

This is a CORS specification violation. Fix by:
- Removing `"*"` from origins and using specific origins, OR
- Setting `allow_credentials: false`

