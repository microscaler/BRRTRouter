# BRRTRouter Local Development with Tilt
# Fast iteration with local builds + minimal container updates

# Set minimum Tilt version
version_settings(constraint='>=0.33.0')

# Configure Tilt web UI port (default: 10350)
# Change this if you have port conflicts
update_settings(k8s_upsert_timeout_secs=60)
config.define_string('tilt_port', args=False, usage='Port for Tilt web UI')
cfg = config.parse()
tilt_port = cfg.get('tilt_port', '10351')  # Default to 10351 to avoid common conflicts

# Set the Tilt UI port
os.putenv('TILT_PORT', tilt_port)

# Host-aware build selection via shell script (exception approved)
brr_build_cmd = 'scripts/host-aware-build.sh brr'
pet_build_cmd = 'scripts/host-aware-build.sh pet'

# ============================================================================
# LOCAL BUILDS (Fast incremental compilation on host)
# ============================================================================

# 0. Build sample-ui (SolidJS + Tailwind)
# Builds the rich dashboard UI and outputs to examples/pet_store/static_site
local_resource(
    'build-sample-ui',
    'cd sample-ui && npm install && npm run build:petstore',
    deps=[
        'sample-ui/src/',
        'sample-ui/index.html',
        'sample-ui/vite.config.js',
        'sample-ui/tailwind.config.js',
        'sample-ui/postcss.config.js',
    ],
    labels=['ui'],
    allow_parallel=True,
)

# 1. Build BRRTRouter library locally for x86_64 Linux (cross-compile from Apple Silicon with zig)
local_resource(
    'build-brrtrouter',
    brr_build_cmd,
    deps=['src/', 'Cargo.toml', 'Cargo.lock'],
    labels=['build'],
    allow_parallel=True,
)

# 2. Generate pet_store from OpenAPI spec locally
local_resource(
    'gen-petstore',
    'cargo run --release --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force',
    deps=['examples/openapi.yaml', 'templates/', 'src/generator/'],
    resource_deps=['build-brrtrouter'],
    labels=['build'],
    allow_parallel=False,  # Must complete before petstore build
)

# =============================================================================
# 3. Build pet_store binary locally and copy to staging
# =============================================================================
# CRITICAL PATTERN: Build Locally → Stage → Docker Copies
# This same pattern is used in tests/curl_harness.rs for curl integration tests!
#
# THE PATTERN:
# -----------
# 1. Build locally:  cargo zigbuild → target/x86_64-unknown-linux-musl/release/pet_store
# 2. Stage locally:  cp → build_artifacts/pet_store  
# 3. Docker copies:  COPY build_artifacts/pet_store → /pet_store (see dockerfiles/Dockerfile.dev)
#
# WHY THIS APPROACH?
# -----------------
# ✅ Fast iteration:  Incremental compilation (10-30s vs 5-10min in Docker)
# ✅ Cargo cache:     Preserved on host between runs
# ✅ Cross-compile:   cargo-zigbuild handles Linux x86_64 from macOS ARM64
# ✅ Quick Docker:    Image build is <1s (just file copies)
# ✅ Always current:  Can't accidentally deploy/test stale code
#
# WHY build_artifacts/ STAGING?
# ----------------------------
# Docker's .dockerignore blocks target/* for performance:
#   target/*                    ← blocks all of target/ (GB of build cache)
#   !build_artifacts/pet_store  ← but explicitly allows this file
#
# Without staging to build_artifacts/:
#   - Docker can't access target/x86_64-unknown-linux-musl/release/pet_store
#   - Build fails even though file exists on host!
#
# FOR FUTURE AI/CONTRIBUTORS:
# --------------------------
# - tests/curl_harness.rs uses the SAME pattern (see STEP 4 comments there)
# - dockerfiles/Dockerfile.test also documents this pattern extensively
# - Do NOT remove staging step (cp to build_artifacts/)
# - Do NOT try to COPY directly from target/ in Dockerfile
# - Do NOT modify .dockerignore to allow target/*
# =============================================================================
local_resource(
    'build-petstore',
    pet_build_cmd + ' && mkdir -p build_artifacts && cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/',
    deps=['examples/pet_store/src/', 'examples/pet_store/Cargo.toml'],
    resource_deps=['gen-petstore'],
    labels=['build'],
    allow_parallel=False,
)

# ============================================================================
# DOCKER IMAGE (Minimal runtime-only image with pre-built binary)
# ============================================================================

# Build Docker image with proper dependencies
# Split into separate stages to ensure correct ordering
local_resource(
    'docker-build-and-push',
    # Build and push to local registry (much faster than 'kind load')
    # https://kind.sigs.k8s.io/docs/user/local-registry/
    # --rm and --force-rm prevent <none>:<none> intermediate container accumulation
    'docker build -t localhost:5001/brrtrouter-petstore:tilt --rm --force-rm -f dockerfiles/Dockerfile.dev . && docker push localhost:5001/brrtrouter-petstore:tilt',
    deps=[
        './build_artifacts/pet_store',
        './examples/pet_store/config',
        './examples/pet_store/doc',
        './examples/pet_store/static_site',
        './dockerfiles/Dockerfile.dev',
    ],
    resource_deps=[
        'build-sample-ui',  # ← UI must be built before Docker image
        'build-petstore',   # ← CRITICAL: Binary must be built first
    ],
    labels=['build'],
    allow_parallel=False,
)

# Tell Tilt about the image from local registry
# The image was already pushed by docker-build-and-push, so we just tag it
custom_build(
    'localhost:5001/brrtrouter-petstore',
    'docker tag localhost:5001/brrtrouter-petstore:tilt $EXPECTED_REF && docker push $EXPECTED_REF',
    deps=[
        './build_artifacts',
        './examples/pet_store/config',
        './examples/pet_store/doc',
        './examples/pet_store/static_site',
    ],
    tag='tilt',
    # Live update: sync files without full rebuild to writable /app directory
    live_update=[
        sync('./build_artifacts/pet_store', '/app/pet_store'),
        sync('./examples/pet_store/config/', '/app/config/'),
        sync('./examples/pet_store/doc/', '/app/doc/'),
        sync('./examples/pet_store/static_site/', '/app/static_site/'),
        run('kill -HUP 1', trigger=['./build_artifacts/pet_store']),
    ],
)

# ============================================================================
# KUBERNETES RESOURCES
# ============================================================================

# ============================================================================
# Load Core Infrastructure
# ============================================================================
k8s_yaml([
    'k8s/core/namespace.yaml',           # Application namespace
    'k8s/velero/namespace.yaml',         # Backup system namespace
])

# ============================================================================
# Load Velero Backup System (Optional)
# ============================================================================
# Note: Run 'just download-velero-crds' once to get velero-crds.yaml
velero_enabled = os.path.exists('k8s/velero/crds.yaml')
if velero_enabled:
    k8s_yaml('k8s/velero/crds.yaml')
    k8s_yaml([
        'k8s/velero/credentials.yaml',
        'k8s/velero/deployment.yaml',
        'k8s/velero/backups.yaml',       # Automated backup schedules
    ])
    print('✅ Velero backup system loaded with automated schedules')
else:
    print('ℹ️  [OPTIONAL] Velero CRDs not found. Run: just download-velero-crds to enable backups')

# ============================================================================
# Load Data Stores (PostgreSQL, Redis) - Start First
# ============================================================================
k8s_yaml([
    'k8s/data/postgres.yaml',
    'k8s/data/redis.yaml',
])

# ============================================================================
# Load Observability Stack
# ============================================================================
# Load storage (PVCs) first
k8s_yaml('k8s/observability/storage.yaml')

# Load observability services
k8s_yaml([
    'k8s/observability/prometheus.yaml',
    'k8s/observability/loki.yaml',
    'k8s/observability/promtail.yaml',
    'k8s/observability/grafana.yaml',
    'k8s/observability/grafana-dashboard.yaml',
    'k8s/observability/jaeger.yaml',
    'k8s/observability/otel-collector.yaml',
])

# ============================================================================
# Load Application (Pet Store)
# ============================================================================
k8s_yaml([
    'k8s/app/deployment.yaml',
    'k8s/app/service.yaml',
])

# ============================================================================
# RESOURCE CONFIGURATION
# ============================================================================

# Velero backup system - independent, starts early (only if enabled)
if velero_enabled:
    k8s_resource(
        'velero',
        labels=['backup'],
    )

# Data stores - start first
k8s_resource(
    'postgres',
    port_forwards=['5432:5432'],
    labels=['data'],
)

k8s_resource(
    'redis',
    port_forwards=['6379:6379'],
    labels=['data'],
)

# Observability stack - independent of data stores
k8s_resource(
    'prometheus',
    port_forwards=['9090:9090'],
    labels=['observability'],
)

k8s_resource(
    'loki',
    port_forwards=['3100:3100'],
    labels=['observability'],
)

k8s_resource(
    'promtail',
    resource_deps=['loki'],
    labels=['observability'],
)

k8s_resource(
    'grafana',
    port_forwards=['3000:3000'],
    resource_deps=['prometheus', 'loki', 'jaeger'],
    labels=['observability'],
)

k8s_resource(
    'jaeger',
    port_forwards=['16686:16686'],
    resource_deps=['postgres', 'redis'],
    labels=['observability'],
)

k8s_resource(
    'otel-collector',
    resource_deps=['jaeger', 'prometheus', 'loki'],
    labels=['observability'],
)

# Petstore application - depends on everything
k8s_resource(
    'petstore',
    port_forwards=['8080:8080'],
    resource_deps=[
        'docker-build-and-push',  # Image must be built and pushed FIRST
        'postgres',
        'redis',
        'prometheus',
        'otel-collector'
    ],
    labels=['app'],
    # Auto-reconnect on pod restart
    auto_init=True,
    trigger_mode=TRIGGER_MODE_AUTO,
)

# ============================================================================
# HELPFUL COMMANDS
# ============================================================================

# Button to manually regenerate petstore code
local_resource(
    'regenerate-petstore',
    'cargo run --release --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force',
    deps=['examples/openapi.yaml', 'templates/'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    labels=['tools'],
    auto_init=False,
)

# Button to run curl tests
local_resource(
    'run-curl-tests',
    'just curls',
    resource_deps=['petstore'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    labels=['tools'],
    auto_init=False,
)

# Button to run standard Goose API load test (all endpoints)
local_resource(
    'run-goose-api-test',
    'cargo run --release --example api_load_test -- --host http://localhost:8080 --users 10 --hatch-rate 2 --run-time 30s',
    resource_deps=['petstore'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    labels=['tools'],
    auto_init=False,
)


# Button to run adaptive Goose load test (finds breaking point via Prometheus)
# Uses optimized defaults: START_USERS=100, STAGE_DURATION=60s, HATCH_RATE=1000
# Customize with env vars: START_USERS, RAMP_STEP, HATCH_RATE, STAGE_DURATION, MAX_USERS
local_resource(
    'run-goose-adaptive',
    'cargo run --release --example adaptive_load_test -- --host http://localhost:8080',
    resource_deps=['petstore'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    labels=['tools'],
    auto_init=False,
)

# ============================================================================
# DISPLAY INFORMATION
# ============================================================================

print("""
╔══════════════════════════════════════════════════════════════════════╗
║                 BRRTRouter Local Development                         ║
╚══════════════════════════════════════════════════════════════════════╝

🚀 Services will be available at:

  📦 Pet Store API:    http://localhost:8080
  📊 Grafana:          http://localhost:3000 (admin/admin - includes Loki/Jaeger datasources)
  📈 Prometheus:       http://localhost:9090
  📋 Loki (logs):      http://localhost:3100
  🔍 Jaeger UI:        http://localhost:16686
  🎛️  Tilt Dashboard:   http://localhost:{tilt_port} (press 'space' to open)
  
  🗄️  PostgreSQL:       localhost:5432 (user: brrtrouter, db: brrtrouter)
  🔴 Redis:            localhost:6379""".format(tilt_port=tilt_port) + """

🏗️  Startup Order:
  1. PostgreSQL & Redis (data stores)
  2. Prometheus, Loki + Promtail, Grafana, Jaeger, OTEL Collector (observability)
  3. Pet Store API (application)

🔧 Quick Actions (in Tilt UI):
  - Click "regenerate-petstore" to rebuild from OpenAPI spec
  - Click "run-curl-tests" to test all endpoints
  - Click "run-goose-api-test" to run standard API load test (all endpoints)
  - Click "run-goose-adaptive" to auto-find breaking point (Prometheus-driven)

📝 Fast Iteration Workflow:
  1. Edit Rust source code
  2. Save file (cargo build runs automatically)
  3. Binary syncs to container (~1-2 seconds)
  4. Service restarts with new code

💡 Tips:
  - Logs are streaming in real-time in Tilt UI
  - Changes to OpenAPI spec trigger automatic regeneration
  - Press 'space' in terminal to open Tilt web UI
  - PostgreSQL and Redis are ready for controllers to use

════════════════════════════════════════════════════════════════════════
""")

