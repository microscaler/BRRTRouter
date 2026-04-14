# CORS Credentials and Authentication Flows

## Overview

This document explains how `allowCredentials: true` works with various authentication mechanisms (SAML, OAuth2, Social Auth) and provides guidance for configuring CORS correctly.

## CORS `allowCredentials: true` - What It Does

When `Access-Control-Allow-Credentials: true` is set, the browser will send:

1. **Cookies** - Including session cookies, CSRF tokens, authentication cookies
2. **Authorization Headers** - Including `Authorization: Bearer <token>`
3. **Client Certificates** - If configured

**Critical CORS Rule**: When `allowCredentials: true`, you **CANNOT** use wildcard origin (`*`). You must specify exact origins.

## Authentication Flow Types

### 1. Redirect-Based Flow (SAML, OAuth2 Authorization Code, Social Login)

**Flow:**
```
1. User clicks "Login with Google"
2. Browser redirects to: https://accounts.google.com/oauth/authorize?...
3. User authenticates
4. Google redirects back to: https://yourapp.com/callback?code=...
5. Your backend exchanges code for tokens
6. Tokens stored (cookie or localStorage)
7. Subsequent API calls use tokens
```

**CORS Impact:**
- **Steps 1-5**: No CORS involved (full page redirects)
- **Step 6**: Token storage (may involve CORS if using API to store)
- **Step 7**: API calls with tokens **REQUIRE** `allowCredentials: true`

### 2. Token-Based Flow (JWT, OAuth2 Access Tokens)

**Flow:**
```
1. User authenticates (redirect or popup)
2. Backend returns token (JWT, OAuth2 access token)
3. Frontend stores token (cookie or localStorage)
4. Frontend sends token in Authorization header: Authorization: Bearer <token>
5. Backend validates token on each request
```

**CORS Impact:**
- **Step 4**: Sending `Authorization` header in cross-origin request **REQUIRES** `allowCredentials: true`
- **Step 3**: If storing in cookie, also requires `allowCredentials: true`

### 3. Cookie-Based Session Flow

**Flow:**
```
1. User authenticates
2. Backend sets HttpOnly cookie: Set-Cookie: session=abc123; HttpOnly; Secure
3. Browser automatically sends cookie on subsequent requests
4. Backend validates session cookie
```

**CORS Impact:**
- **Step 3**: Sending cookies in cross-origin request **REQUIRES** `allowCredentials: true`
- Cookie must have `SameSite=None; Secure` for cross-origin

## BRRTRouter Authentication Support

BRRTRouter supports the following authentication methods:

### 1. Bearer JWT (Authorization Header)

```yaml
# OpenAPI
securitySchemes:
  BearerAuth:
    type: http
    scheme: bearer

# config.yaml
security:
  jwks:
    BearerAuth:
      jwks_url: "https://auth.example.com/.well-known/jwks.json"
      iss: "https://auth.example.com/"
      aud: "my-api"
```

**CORS Requirements:**
- ✅ `allowCredentials: true` (to send Authorization header)
- ✅ Specific origins (not wildcard)

### 2. OAuth2 (Authorization Code Flow)

```yaml
# OpenAPI
securitySchemes:
  OAuth2:
    type: oauth2
    flows:
      authorizationCode:
        authorizationUrl: https://auth.example.com/authorize
        tokenUrl: https://auth.example.com/token

# config.yaml
security:
  jwks:
    OAuth2:
      jwks_url: "https://auth.example.com/.well-known/jwks.json"
```

**CORS Requirements:**
- ✅ `allowCredentials: true` (to send Authorization header with access token)
- ✅ Specific origins (not wildcard)
- ✅ Token exchange happens server-side (no CORS for that step)

### 3. API Key (Cookie-Based)

```yaml
# OpenAPI
securitySchemes:
  ApiKeyCookie:
    type: apiKey
    in: cookie
    name: session

# config.yaml
security:
  api_keys:
    ApiKeyCookie:
      key: "secret-key"
```

**CORS Requirements:**
- ✅ `allowCredentials: true` (to send cookie)
- ✅ Specific origins (not wildcard)
- ✅ Cookie must have `SameSite=None; Secure` for cross-origin

### 4. SAML (via JWT after SAML assertion)

SAML typically works like this:

```
1. User redirected to SAML IdP
2. User authenticates
3. IdP sends SAML assertion to your backend
4. Backend validates assertion and issues JWT
5. JWT sent to frontend (cookie or token)
6. Frontend uses JWT for API calls
```

**CORS Requirements:**
- ✅ `allowCredentials: true` (for step 6 - sending JWT)
- ✅ Specific origins (not wildcard)
- Steps 1-4 don't involve CORS (server-side)

## Configuration Examples

### Example 1: OAuth2 with JWT Tokens (Authorization Header)

```yaml
# config.yaml
cors:
  origins:
    - "https://app.example.com"      # Production frontend
    - "http://localhost:3000"         # Local dev
  allow_credentials: true             # Required for Authorization header
  allowed_headers:
    - "Content-Type"
    - "Authorization"                  # Required for Bearer tokens
  allowed_methods:
    - "GET"
    - "POST"
    - "PUT"
    - "DELETE"
    - "OPTIONS"

# OpenAPI - route-specific
/pets:
  get:
    x-cors:
      allowCredentials: true
      allowedHeaders:
        - "Authorization"
```

### Example 2: Cookie-Based Session

```yaml
# config.yaml
cors:
  origins:
    - "https://app.example.com"
  allow_credentials: true             # Required for cookies
  allowed_headers:
    - "Content-Type"
    - "X-CSRF-Token"                   # If using CSRF protection
  allowed_methods:
    - "GET"
    - "POST"
    - "OPTIONS"

# Backend must set cookie with:
# Set-Cookie: session=abc123; HttpOnly; Secure; SameSite=None
```

### Example 3: Social Auth (Google, GitHub, etc.)

Social auth typically uses OAuth2:

```yaml
# config.yaml
cors:
  origins:
    - "https://app.example.com"
  allow_credentials: true             # Required for token in Authorization header
  allowed_headers:
    - "Content-Type"
    - "Authorization"
  allowed_methods:
    - "GET"
    - "POST"
    - "OPTIONS"

# OpenAPI
securitySchemes:
  GoogleOAuth:
    type: oauth2
    flows:
      authorizationCode:
        authorizationUrl: https://accounts.google.com/o/oauth2/v2/auth
        tokenUrl: https://oauth2.googleapis.com/token
```

## Common Issues and Solutions

### Issue 1: "Credentials flag is true, but Access-Control-Allow-Origin is *"

**Error:**
```
Access to fetch at 'https://api.example.com/pets' from origin 'https://app.example.com' 
has been blocked by CORS policy: The value of the 'Access-Control-Allow-Credentials' 
header in the response is 'true', but the value of the 'Access-Control-Allow-Origin' 
header is '*', which is not allowed.
```

**Solution:**
- Remove wildcard (`*`) from origins
- Specify exact origins in `config.yaml`:
  ```yaml
  cors:
    origins:
      - "https://app.example.com"  # Not "*"
  ```

### Issue 2: Cookies Not Sent Cross-Origin

**Symptom:** Cookies are not sent with cross-origin requests even with `allowCredentials: true`

**Solution:**
- Ensure cookie has `SameSite=None; Secure`:
  ```rust
  Set-Cookie: session=abc123; HttpOnly; Secure; SameSite=None
  ```
- Ensure `allowCredentials: true` in CORS config
- Ensure specific origins (not wildcard)

### Issue 3: Authorization Header Not Sent

**Symptom:** `Authorization: Bearer <token>` header is not sent

**Solution:**
- Ensure `allowCredentials: true` in CORS config
- Ensure `Authorization` is in `allowed_headers`:
  ```yaml
  cors:
    allowed_headers:
      - "Authorization"
  ```

## Security Best Practices

1. **Always use specific origins** - Never use wildcard (`*`) with credentials
2. **Use HttpOnly cookies** - Prevents XSS attacks
3. **Use Secure flag** - Only send cookies over HTTPS
4. **Use SameSite=None** - Required for cross-origin cookies
5. **Validate origins** - Only allow trusted frontend domains
6. **Short token TTL** - Use short-lived tokens (15-60 minutes)
7. **Refresh tokens** - Use refresh tokens for long-lived sessions

## Testing

### Test CORS with Credentials

```bash
# Test preflight
curl -X OPTIONS https://api.example.com/pets \
  -H "Origin: https://app.example.com" \
  -H "Access-Control-Request-Method: GET" \
  -H "Access-Control-Request-Headers: Authorization" \
  -v

# Should return:
# Access-Control-Allow-Origin: https://app.example.com
# Access-Control-Allow-Credentials: true
# Access-Control-Allow-Headers: Authorization

# Test actual request
curl -X GET https://api.example.com/pets \
  -H "Origin: https://app.example.com" \
  -H "Authorization: Bearer <token>" \
  -H "Cookie: session=abc123" \
  -v
```

## Summary

- **`allowCredentials: true` is REQUIRED** for:
  - Sending `Authorization` headers (Bearer tokens, OAuth2 tokens)
  - Sending cookies (session cookies, CSRF tokens)
  - Any authentication that requires credentials in cross-origin requests

- **Wildcard origin (`*`) is FORBIDDEN** when `allowCredentials: true`

- **SAML/Social Auth** typically:
  1. Use redirect-based flow (no CORS for initial auth)
  2. Return tokens (JWT, OAuth2 access token)
  3. Frontend sends tokens in `Authorization` header (requires CORS credentials)
  4. Or stores tokens in cookies (requires CORS credentials)

- **BRRTRouter supports** all these flows via:
  - `JwksBearerProvider` for OAuth2/OIDC/SAML (via JWT)
  - `BearerJwtProvider` for simple JWT validation
  - `OAuth2Provider` for OAuth2 token validation
  - Cookie-based API keys

