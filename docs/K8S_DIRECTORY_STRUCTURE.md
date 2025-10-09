# Kubernetes Directory Structure

## Overview

The `k8s/` directory is organized by logical system groupings for easy navigation and maintenance.

## Directory Structure

```
k8s/
├── app/                    # Application (Pet Store)
│   ├── deployment.yaml     # Pet Store deployment
│   ├── service.yaml        # Pet Store service
│   └── pvc.yaml           # Pet Store persistent storage
│
├── core/                   # Core cluster infrastructure
│   ├── namespace.yaml      # Application namespace (brrtrouter-dev)
│   └── local-registry-hosting.yaml  # Registry proxy configuration
│
├── data/                   # Data stores
│   ├── postgres.yaml       # PostgreSQL database
│   └── redis.yaml          # Redis cache
│
├── observability/          # Monitoring & logging stack
│   ├── prometheus.yaml     # Metrics collection
│   ├── grafana.yaml        # Dashboards
│   ├── grafana-dashboard.yaml  # Dashboard configuration
│   ├── loki.yaml          # Log aggregation
│   ├── promtail.yaml      # Log shipping
│   ├── jaeger.yaml        # Distributed tracing
│   ├── otel-collector.yaml # OpenTelemetry collector
│   └── storage.yaml       # PVCs for observability stack
│
└── velero/                 # Backup system
    ├── namespace.yaml      # Velero namespace
    ├── credentials.yaml    # MinIO credentials
    ├── deployment.yaml     # Velero deployment
    ├── backups.yaml       # Automated backup schedules
    ├── crds.yaml          # Custom Resource Definitions (downloaded)
    └── docker-compose-minio.yml  # MinIO (runs outside KIND)
```

## Organization Principles

### 1. **app/** - Application Components
All application-specific resources for the Pet Store demo application.

**Contains:**
- Deployments
- Services  
- PersistentVolumeClaims
- Application-specific ConfigMaps/Secrets

### 2. **core/** - Core Infrastructure
Fundamental cluster resources that must exist before other systems.

**Contains:**
- Namespaces
- Registry configurations
- Cluster-wide settings

**Loaded first** in Tiltfile to ensure namespaces exist.

### 3. **data/** - Data Stores
Persistent data storage systems.

**Contains:**
- PostgreSQL
- Redis
- Other databases/caches

**Loaded second** in Tiltfile (after core, before observability).

### 4. **observability/** - Monitoring & Logging
Complete observability stack for metrics, logs, and traces.

**Contains:**
- Prometheus (metrics)
- Grafana (dashboards)
- Loki (log aggregation)
- Promtail (log shipping)
- Jaeger (distributed tracing)
- OpenTelemetry Collector
- Persistent storage for observability data

**Loaded third** in Tiltfile (can start in parallel with data stores).

### 5. **velero/** - Backup System
Optional backup and disaster recovery system.

**Contains:**
- Velero deployment
- MinIO configuration (external storage)
- Backup schedules
- Custom Resource Definitions

**Optional** - Only loaded if CRDs are present.

## Tilt Loading Order

The Tiltfile loads resources in dependency order:

```python
# 1. Core Infrastructure
k8s_yaml([
    'k8s/core/namespace.yaml',
    'k8s/velero/namespace.yaml',
])

# 2. Velero (Optional - if CRDs exist)
if velero_enabled:
    k8s_yaml('k8s/velero/crds.yaml')
    k8s_yaml([
        'k8s/velero/credentials.yaml',
        'k8s/velero/deployment.yaml',
        'k8s/velero/backups.yaml',
    ])

# 3. Data Stores
k8s_yaml([
    'k8s/data/postgres.yaml',
    'k8s/data/redis.yaml',
])

# 4. Observability Stack
k8s_yaml('k8s/observability/storage.yaml')  # PVCs first
k8s_yaml([
    'k8s/observability/prometheus.yaml',
    'k8s/observability/loki.yaml',
    'k8s/observability/promtail.yaml',
    'k8s/observability/grafana.yaml',
    'k8s/observability/grafana-dashboard.yaml',
    'k8s/observability/jaeger.yaml',
    'k8s/observability/otel-collector.yaml',
])

# 5. Application
k8s_yaml([
    'k8s/app/deployment.yaml',
    'k8s/app/service.yaml',
])
```

## Benefits

### ✅ Improved Navigation
- Find all Prometheus resources in `k8s/observability/`
- Find all application resources in `k8s/app/`
- Clear separation of concerns

### ✅ Easier Maintenance
- Update all observability configs in one directory
- Add new data stores without cluttering root
- Isolated backup system configuration

### ✅ Better Documentation
- Directory structure is self-documenting
- Easy to explain to new contributors
- Logical groupings match mental models

### ✅ Scalability
- Easy to add new systems (e.g., `k8s/messaging/`)
- Can version individual systems independently
- Clear ownership boundaries

## Adding New Resources

### Adding to Existing System

1. Add manifest to appropriate directory:
   ```bash
   # Add a new Redis sentinel configuration
   touch k8s/data/redis-sentinel.yaml
   ```

2. Update Tiltfile to load it:
   ```python
   k8s_yaml([
       'k8s/data/postgres.yaml',
       'k8s/data/redis.yaml',
       'k8s/data/redis-sentinel.yaml',  # New
   ])
   ```

### Adding New System

1. Create new directory:
   ```bash
   mkdir k8s/messaging
   ```

2. Add manifests:
   ```bash
   touch k8s/messaging/rabbitmq.yaml
   touch k8s/messaging/kafka.yaml
   ```

3. Add to Tiltfile:
   ```python
   # Load messaging system
   k8s_yaml([
       'k8s/messaging/rabbitmq.yaml',
       'k8s/messaging/kafka.yaml',
   ])
   ```

4. Configure resources:
   ```python
   k8s_resource(
       'rabbitmq',
       port_forwards=['5672:5672', '15672:15672'],
       labels=['messaging'],
   )
   ```

## Migration Notes

### Old Structure → New Structure

| Old Path | New Path |
|----------|----------|
| `k8s/namespace.yaml` | `k8s/core/namespace.yaml` |
| `k8s/local-registry-hosting.yaml` | `k8s/core/local-registry-hosting.yaml` |
| `k8s/postgres.yaml` | `k8s/data/postgres.yaml` |
| `k8s/redis.yaml` | `k8s/data/redis.yaml` |
| `k8s/prometheus.yaml` | `k8s/observability/prometheus.yaml` |
| `k8s/grafana.yaml` | `k8s/observability/grafana.yaml` |
| `k8s/grafana-dashboard-unified.yaml` | `k8s/observability/grafana-dashboard.yaml` |
| `k8s/loki.yaml` | `k8s/observability/loki.yaml` |
| `k8s/promtail.yaml` | `k8s/observability/promtail.yaml` |
| `k8s/jaeger.yaml` | `k8s/observability/jaeger.yaml` |
| `k8s/otel-collector.yaml` | `k8s/observability/otel-collector.yaml` |
| `k8s/observability-storage.yaml` | `k8s/observability/storage.yaml` |
| `k8s/petstore-deployment.yaml` | `k8s/app/deployment.yaml` |
| `k8s/petstore-service.yaml` | `k8s/app/service.yaml` |
| `k8s/petstore-pvc.yaml` | `k8s/app/pvc.yaml` |
| `k8s/velero-*.yaml` | `k8s/velero/*.yaml` |

### Files Also Renamed

Some files were renamed for clarity:
- `observability-storage.yaml` → `storage.yaml` (within observability/)
- `grafana-dashboard-unified.yaml` → `grafana-dashboard.yaml`
- `petstore-deployment.yaml` → `deployment.yaml` (within app/)
- `petstore-service.yaml` → `service.yaml` (within app/)
- `petstore-pvc.yaml` → `pvc.yaml` (within app/)

The directory context makes the purpose clear without redundant prefixes.

## See Also

- [LOCAL_DEVELOPMENT.md](LOCAL_DEVELOPMENT.md) - Development workflow
- [TILT_IMPLEMENTATION.md](TILT_IMPLEMENTATION.md) - Tilt configuration details
- [DECLARATIVE_INFRASTRUCTURE_AUDIT.md](DECLARATIVE_INFRASTRUCTURE_AUDIT.md) - Infrastructure audit


