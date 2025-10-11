# Port Mapping Standardization - October 2025

## 🎯 Summary

Fixed confusing port mappings and added external access to PostgreSQL and Redis for better developer experience.

## 📊 Changes Made

### Before (Confusing)
| Service | Localhost Port | Container Port | Issue |
|---------|---------------|----------------|-------|
| Pet Store | 9090 | 8080 | ❌ Non-standard (9090 typically Prometheus) |
| Prometheus | 8080 | 9090 | ❌ Confusing (8080 typically HTTP) |
| PostgreSQL | - | 5432 | ❌ Not exposed externally |
| Redis | - | 6379 | ❌ Not exposed externally |

### After (Standard)
| Service | Localhost Port | Container Port | Standard | Status |
|---------|---------------|----------------|----------|--------|
| Pet Store | **8080** | **8080** | ✅ Standard HTTP | **Fixed** |
| Prometheus | **9090** | **9090** | ✅ Standard Prometheus | **Fixed** |
| Grafana | 3000 | 3000 | ✅ Standard Grafana | OK |
| Jaeger | 16686 | 16686 | ✅ Standard Jaeger | OK |
| PostgreSQL | **5432** | **5432** | ✅ Standard PostgreSQL | **Added** |
| Redis | **6379** | **6379** | ✅ Standard Redis | **Added** |

## 🎯 Benefits

### 1. Industry Standard Ports
- **8080**: Universally recognized as standard HTTP (alternatives to 80)
- **9090**: Default Prometheus port (metrics scraping)
- **5432**: Standard PostgreSQL port
- **6379**: Standard Redis port

### 2. External Database Access
Contributors can now use external tools:

```bash
# PostgreSQL clients (pgAdmin, DBeaver, psql)
psql -h localhost -U brrtrouter -d brrtrouter
# Password: dev_password_change_in_prod

# Redis clients (RedisInsight, redis-cli)
redis-cli -h localhost -p 6379
```

### 3. Less Confusion
- No more "why is the API on 9090?"
- No more "where is Prometheus?"
- Easier to remember: Pet Store = 8080, Prometheus = 9090

### 4. Better Documentation
All URLs now match industry expectations:
- API docs typically say "http://localhost:8080"
- Prometheus docs typically say "http://localhost:9090"

## 📝 Files Updated

### Tiltfile
```diff
 k8s_resource(
+    'postgres',
+    port_forwards=['5432:5432'],
+    labels=['data'],
 )

 k8s_resource(
+    'redis',
+    port_forwards=['6379:6379'],
+    labels=['data'],
 )

 k8s_resource(
     'prometheus',
-    port_forwards=['8080:9090'],
+    port_forwards=['9090:9090'],
     resource_deps=['postgres', 'redis'],
     labels=['observability'],
 )

 k8s_resource(
     'petstore',
-    port_forwards=['9090:8080'],
+    port_forwards=['8080:8080'],
     resource_deps=[
```

### README.md
- Updated Quick Start section with correct URLs
- Updated Service URLs table
- Added PostgreSQL and Redis connection examples

### docs/TILT_SUCCESS.md
- Updated all service URLs
- Added database connection commands
- Fixed port mapping strategy documentation

### docs/CONTRIBUTOR_ONBOARDING.md
- Updated verification steps
- Added PostgreSQL and Redis testing
- Fixed all curl examples

### CONTRIBUTING.md
- Updated load test command to use port 8080

## 🔧 Testing

All services confirmed working on standard ports:

```bash
✅ Pet Store (8080):    {"status":"ok"}
✅ Prometheus (9090):   Prometheus Server is Healthy.
✅ PostgreSQL (5432):   Port open and accepting connections
✅ Redis (6379):        Port open and accepting connections
✅ Grafana (3000):      Dashboard accessible
✅ Jaeger (16686):      UI accessible
```

## 🎓 Use Cases Enabled

### 1. Database GUI Tools
```bash
# Use pgAdmin, DBeaver, TablePlus, DataGrip, etc.
Host: localhost
Port: 5432
User: brrtrouter
Password: dev_password_change_in_prod
Database: brrtrouter
```

### 2. Redis GUI Tools
```bash
# Use RedisInsight, Medis, Redis Desktop Manager, etc.
Host: localhost
Port: 6379
```

### 3. Direct Query Testing
```bash
# Test database schema
psql -h localhost -U brrtrouter -d brrtrouter -c "\\dt"

# Inspect cache
redis-cli -h localhost -p 6379 KEYS "*"
```

### 4. Data Seeding
```bash
# Load test data
psql -h localhost -U brrtrouter -d brrtrouter < test_data.sql

# Populate cache
redis-cli -h localhost -p 6379 < seed_cache.txt
```

### 5. Integration Testing
```bash
# External scripts can now access all services
pytest tests/integration/ \
  --api-url http://localhost:8080 \
  --db-host localhost \
  --db-port 5432 \
  --redis-host localhost \
  --redis-port 6379
```

## 📊 Developer Experience Impact

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| **Port memorization** | Hard (non-standard) | Easy (standard) | ⭐⭐⭐⭐⭐ |
| **Database access** | None | Direct | ⭐⭐⭐⭐⭐ |
| **GUI tools** | Blocked | Enabled | ⭐⭐⭐⭐⭐ |
| **Documentation clarity** | Confusing | Clear | ⭐⭐⭐⭐⭐ |
| **External testing** | Difficult | Simple | ⭐⭐⭐⭐⭐ |

## 🚀 Next Steps for Contributors

To use the new port mappings:

1. **Stop current Tilt** (if running): `Ctrl-C` or `tilt down`
2. **Restart Tilt**: `tilt up`
3. **Verify new ports**:
   ```bash
   curl http://localhost:8080/health
   curl http://localhost:9090/-/healthy
   psql -h localhost -U brrtrouter -d brrtrouter -c "SELECT 1"
   redis-cli -h localhost -p 6379 PING
   ```

## 🎉 Result

BRRTRouter now uses **industry-standard ports** for all services, making it:
- ✅ More intuitive for new contributors
- ✅ Compatible with standard tooling
- ✅ Easier to document and remember
- ✅ Better for integration testing
- ✅ Aligned with production best practices

---

**Status**: ✅ Complete
**Date**: October 9, 2025
**Impact**: High (Developer Experience)

