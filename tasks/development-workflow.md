# BRRTRouter Development Workflow

*A systematic approach to prevent the chaos of accumulating 29 uncommitted files and 2 days of wasted debugging.*

## Overview

This workflow ensures every change is tested immediately, builds are always clean, and we never lose track of what's working vs. broken.

## Phase 1: DEV - Development Setup

### Prerequisites
```bash
# Always start with clean state
git status --porcelain  # Must be empty or contain only known changes
```

### Branch Creation
```bash
# Start from clean main
git checkout main
git pull origin main
git checkout -b feature/descriptive-name

# Load Memory Bank context (MANDATORY)
farm agent startup

# Verify starting state
cargo check --lib
cd examples/pet_store && cargo check && cd ../..
```

### Development Rules
- **ONE logical change per iteration**
- **No more than 5 files modified at once**
- **Document what you're changing and why**

## Phase 2: BUILD - Compilation Verification

### Library Build
```bash
# Core library must compile cleanly
cargo check --lib
# EXIT IMMEDIATELY if any errors
```

### Binary Build
```bash
# All binaries must compile
cargo check --bins
# EXIT IMMEDIATELY if any errors
```

### Example Build
```bash
# Generated code must compile
cd examples/pet_store
cargo check
cd ../..
# EXIT IMMEDIATELY if any errors
```

### Build Success Criteria
- ✅ Zero compilation errors
- ✅ Zero warnings - even unrelated to your changes, you own the whole branch during development!
- ✅ All binaries and examples build

## Phase 3: LINT - Code Quality

### Formatting
```bash
# Check formatting
cargo fmt --check

# Auto-fix if needed
cargo fmt
```

### Linting
```bash
# Run clippy with strict warnings
cargo clippy -- -D warnings
# Must pass with zero warnings
```

### Dependency Check
```bash
# Check for unused dependencies (if available)
cargo machete
```

### Lint Success Criteria
- ✅ Code is properly formatted
- ✅ Zero clippy warnings
- ✅ No unused dependencies

## Phase 4: UNIT TEST - Automated Testing

### Core Tests
```bash
# Run library tests
cargo test --lib
```

### Module-Specific Tests
```bash
# Run relevant module tests
cargo test --test generator_tests
cargo test --test template_validation_tests
# Add others as relevant to your changes
```

### Coverage Check
```bash
# Check test coverage using farm tools
farm coverage python
# Must maintain >65% coverage, target 80%
```

### Integration Tests
```bash
# Run full integration test suite
cargo test --tests
```

### Test Success Criteria
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ Coverage maintained or improved
- ✅ No flaky test failures

## Phase 5: CURL TEST - Complete API Validation

### Service Generation
```bash
# Generate fresh service from OpenAPI spec
cargo run --bin brrtrouter-gen -- generate --spec examples/pet_store/doc/openapi.yaml --force
```

### Service Startup
```bash
# Start service using proper management
python scripts/manage_service.py start

# Wait for startup
sleep 3

# Verify service is running
python scripts/manage_service.py status
```

### Comprehensive Endpoint Testing

#### Infrastructure Endpoints
```bash
echo "=== TESTING INFRASTRUCTURE ==="
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/health
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/docs
```

#### Pet Endpoints
```bash
echo "=== TESTING PET ENDPOINTS ==="
# GET /pets - List all pets
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/pets

# GET /pets/{petId} - Get specific pet
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/pets/1

# POST /pets - Add new pet
curl -s -w "Status: %{http_code}\n" -X POST -H "X-API-Key: test123" -H "Content-Type: application/json" \
  -d '{"name":"Fluffy","status":"available"}' http://localhost:8080/pets
```

#### User Endpoints
```bash
echo "=== TESTING USER ENDPOINTS ==="
# GET /users - List all users
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/users

# GET /users/{user_id} - Get specific user
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/users/123

# GET /users/{user_id}/posts - List user posts
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/users/123/posts
```

#### Post Endpoints
```bash
echo "=== TESTING POST ENDPOINTS ==="
# GET /posts/{post_id} - Get specific post
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/posts/456
```

#### Item Endpoints
```bash
echo "=== TESTING ITEM ENDPOINTS ==="
# GET /items/{item_id} - Get specific item
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/items/789

# POST /items - Create new item
curl -s -w "Status: %{http_code}\n" -X POST -H "X-API-Key: test123" -H "Content-Type: application/json" \
  -d '{"name":"Test Item","category":"electronics"}' http://localhost:8080/items
```

#### Admin Endpoints
```bash
echo "=== TESTING ADMIN ENDPOINTS ==="
# GET /admin/settings - Admin settings
curl -s -w "Status: %{http_code}\n" -H "X-API-Key: test123" http://localhost:8080/admin/settings
```

#### SSE Endpoints
```bash
echo "=== TESTING SSE ENDPOINTS ==="
# GET /events - Server-sent events (test for 3 seconds max)
timeout 3s curl -s -H "X-API-Key: test123" http://localhost:8080/events || echo "SSE test completed"
```

#### Authentication Tests
```bash
echo "=== TESTING AUTHENTICATION ==="
# Test without API key (should get 401)
NO_AUTH=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:8080/pets)
echo "No auth: $NO_AUTH (should be 401)"

# Test with invalid API key (should get 401)
INVALID_AUTH=$(curl -s -w "%{http_code}" -o /dev/null -H "X-API-Key: invalid" http://localhost:8080/pets)
echo "Invalid auth: $INVALID_AUTH (should be 401)"
```

### Response Validation
```bash
echo "=== VALIDATING JSON RESPONSES ==="
# Validate JSON structure for key endpoints
curl -s -H "X-API-Key: test123" http://localhost:8080/pets | jq empty || { echo "❌ Invalid JSON from /pets"; exit 1; }
curl -s -H "X-API-Key: test123" http://localhost:8080/users | jq empty || { echo "❌ Invalid JSON from /users"; exit 1; }
curl -s -H "X-API-Key: test123" http://localhost:8080/users/123 | jq empty || { echo "❌ Invalid JSON from /users/123"; exit 1; }
```

### Service Cleanup
```bash
# Always stop service cleanly
python scripts/manage_service.py stop
```

### CURL Success Criteria
- ✅ All endpoints return expected HTTP status codes
- ✅ All JSON responses are valid
- ✅ Authentication works correctly (401 for invalid/missing keys)
- ✅ Service starts and stops cleanly
- ✅ No runtime panics or crashes

## Phase 6: COMMIT & ITERATE

### Commit Working State
```bash
# Add all changes
git add .

# Commit with descriptive message
git commit -m "feat: [specific change made]

- Bullet point describing what was changed
- Another bullet describing why
- Verification that all tests pass
- Note any coverage changes"

# Push immediately
git push origin feature/branch-name
```

### Update Memory Bank
```bash
# Save working state to Memory Bank
farm agent memory-bank update "Successfully implemented [change], all phases pass"
```

### Continue or Complete
- **Continue**: Make next small change and repeat from Phase 2
- **Complete**: Create pull request with comprehensive testing results

## Mandatory Rules

### Never Skip Rules
1. **Never skip BUILD phase** - If it doesn't compile, stop everything
2. **Never commit failing tests** - Fix or explicitly skip broken tests
3. **Never accumulate more than 5 file changes** - Commit working increments
4. **Never bypass service management** - Always use `python scripts/manage_service.py`
5. **Never assume tests pass** - Run them every time

### Failure Response
If any phase fails:
1. **STOP immediately** - Don't proceed to next phase
2. **Isolate the problem** - Identify minimum change to fix
3. **Fix only that problem** - Don't add other changes
4. **Re-run from BUILD phase** - Verify fix works
5. **Commit working state** - Save progress immediately

### Emergency Recovery
If you're lost in a broken state:
```bash
# Check what's changed
git status --porcelain

# If too many changes (>10 files), consider reset
git stash
git checkout main
git checkout -b feature/clean-restart

# Start over with this workflow
```

## Tools Integration

### Farm CLI Usage
```bash
# Always start sessions
farm agent startup

# Use for testing
farm test python test_file.py

# Use for coverage
farm coverage python

# Use for linting
farm lint python --fix
```

### Expected Outputs
Each phase should produce clear success/failure indicators:
- **BUILD**: "Finished dev profile [unoptimized + debuginfo] target(s)"
- **LINT**: "All checks passed"
- **TEST**: "test result: ok. X passed; 0 failed"
- **CURL**: "✅ All endpoints responding correctly"

## Success Metrics

### Per-Change Metrics
- Time from start to commit: < 30 minutes for small changes
- Number of files modified: ≤ 5 per commit
- Test coverage: Maintained or improved
- Compilation time: No significant regression

### Overall Project Health
- All phases pass consistently
- No accumulation of technical debt
- Clear commit history with working states
- Reproducible development environment

---

**Remember**: This workflow exists because we learned the hard way that skipping steps leads to 2 days of wasted debugging. Follow it religiously. 