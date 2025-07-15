# BRRTRouter Scripts

This directory contains utility scripts for managing and testing the BRRTRouter service.

## Scripts Overview

### 🧪 `test_api.py` - Pragmatic API Testing

A human-friendly API testing script that provides clear, colorful output and comprehensive test coverage.

**Features:**
- 🎯 **Human-readable output** with colored terminal formatting
- 📋 **Structured test categories**: Infrastructure, Pets API, Users API, Authentication
- 📊 **Detailed request/response logging** showing HTTP method, path, status, and response content
- 📈 **Comprehensive test summary** with success rates and failure analysis
- 🔍 **Smart validation** of response structure and required fields
- ⚡ **Fast execution** with clear progress indicators

**Usage:**
```bash
# Test the default local service
python scripts/test_api.py

# Test a different server
python scripts/test_api.py http://staging.example.com:8080

# Use a different OpenAPI spec
python scripts/test_api.py http://localhost:8080 path/to/openapi.yaml
```

**Sample Output:**
```
============================================================
  BRRTRouter API Test Suite
============================================================
🎯 Testing: http://localhost:8080
📋 Spec: examples/pet_store/doc/openapi.yaml
✅ Server is running and healthy

📋 Infrastructure Tests
----------------------------------------
🧪 Health Check
→ GET /health
← 🟢 200 OK
{"status":"ok"}
✅ PASS

📋 Pets API Tests  
----------------------------------------
🧪 List Pets
→ GET /pets
  Query params: {'limit': 5}
← 🟢 200 OK
[
  {
    "id": 12345,
    "name": "Max",
    "breed": "Golden Retriever",
    "age": 3
  }
]
✅ PASS - Retrieved 2 pets
✅ All required fields present

====== Test Summary ======
Category           Count    Percentage
----------------------------------------
✅ Passed          8        80.0%
❌ Failed          1        10.0%
🔧 Validation      1        10.0%
----------------------------------------
TOTAL              10       100%

🎉 Overall Status: EXCELLENT (80.0% success rate)
```

### 🚀 `manage_service.py` - Service Management

Manages the BRRTRouter service using macOS launchd for proper process management.

**Usage:**
```bash
# Start the service
python scripts/manage_service.py start

# Check service status
python scripts/manage_service.py status

# View recent logs
python scripts/manage_service.py logs

# Restart the service
python scripts/manage_service.py restart

# Stop the service
python scripts/manage_service.py stop
```

### 📋 `test_openapi_spec.py` - Comprehensive OpenAPI Testing

A more detailed testing framework that dynamically generates tests from the OpenAPI specification.

**Usage:**
```bash
python scripts/test_openapi_spec.py
```

## Request Logging

When the service is running with the updated templates, you'll see detailed request logs:

```
🚀 Starting pet_store server...
📋 OpenAPI spec: doc/openapi.yaml
🌐 Server will start on http://localhost:8080
🔑 Test API key: test123
📊 Request logs will be shown below:
============================================================

→ GET /health (health_endpoint)
← 🟢 200 /health 12ms

→ GET /pets (list_pets)
← 🟢 200 /pets 45ms

→ POST /pets (add_pet)
← 🟢 201 /pets 67ms
```

**Log Format:**
- `→` indicates incoming requests with method, path, and handler name
- `←` indicates responses with status code, path, and response time
- 🟢 Green for success (2xx), 🟡 Yellow for redirects (3xx), 🔴 Red for errors (4xx/5xx)

## Development Workflow

1. **Start the service**: `python scripts/manage_service.py start`
2. **Test the API**: `python scripts/test_api.py`
3. **View logs**: `python scripts/manage_service.py logs`
4. **Make changes and restart**: `python scripts/manage_service.py restart`

## Dependencies

- **requests**: For HTTP client functionality
- **yaml**: For OpenAPI specification parsing
- **Standard Python libraries**: json, sys, time, pathlib, etc.

No external dependencies required for the basic functionality - the test script uses standard Python libraries with simple terminal coloring.

## Configuration

### Test Script Configuration

Edit `scripts/test_api.py` to customize:
- Default base URL
- Default OpenAPI spec path
- Test timeouts
- Additional test cases

### Service Configuration  

The service is configured via `scripts/com.brrtrouter.serve.plist` with:
- Hot reload enabled
- Test API key: `test123`
- Port: 8080
- Proper logging and error handling

## Troubleshooting

### Service Won't Start
```bash
# Check logs for errors
python scripts/manage_service.py logs

# Try manual start for debugging
cd examples/pet_store
cargo run -- --spec doc/openapi.yaml --port 8080 --test-api-key test123
```

### API Tests Failing
```bash
# Verify service is running
python scripts/manage_service.py status

# Test health endpoint manually
curl http://localhost:8080/health

# Check authentication
curl -H "X-API-Key: test123" http://localhost:8080/pets
```

### Connection Refused
- Ensure the service is started: `python scripts/manage_service.py start`
- Check if another process is using port 8080: `lsof -i :8080`
- Verify the service compiled successfully in the logs 