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

# ============================================================================
# LOCAL BUILDS (Fast incremental compilation on host)
# ============================================================================

# 0. Build sample-ui (SolidJS + Tailwind) - Vite builds directly to target
local_resource(
    'build-sample-ui',
    'cd sample-ui && yarn install && yarn build',
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
    'cargo zigbuild --release --target x86_64-unknown-linux-musl --lib',
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

# 3. Build pet_store binary locally and copy to staging (fast incremental compilation for x86_64 Linux with zig)
local_resource(
    'build-petstore',
    'cargo zigbuild --release --target x86_64-unknown-linux-musl -p pet_store && mkdir -p build_artifacts && cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/',
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
    'docker-build-and-load',
    # Wait for builds, then build and load image atomically
    'docker build -t brrtrouter-petstore:tilt -f Dockerfile.dev . && kind load docker-image brrtrouter-petstore:tilt --name brrtrouter-dev',
    deps=[
        './build_artifacts/pet_store',
        './examples/pet_store/config',
        './examples/pet_store/doc',
        './examples/pet_store/static_site',
        './Dockerfile.dev',
    ],
    resource_deps=[
        'build-sample-ui',  # ← CRITICAL: UI must be built and copied first
        'build-petstore',   # ← CRITICAL: Binary must be built first
    ],
    labels=['build'],
    allow_parallel=False,
)

# Tell Kubernetes about the image
custom_build(
    'brrtrouter-petstore',
    'docker tag brrtrouter-petstore:tilt $EXPECTED_REF',
    deps=[
        './build_artifacts',
        './examples/pet_store/config',
        './examples/pet_store/doc',
        './examples/pet_store/static_site',
    ],
    tag='tilt',
    disable_push=True,
    skips_local_docker=True,
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

# Load namespace first
k8s_yaml('k8s/namespace.yaml')

# Load data stores (PostgreSQL, Redis) - these must start first
k8s_yaml([
    'k8s/postgres.yaml',
    'k8s/redis.yaml',
])

# Load observability stack
k8s_yaml([
    'k8s/prometheus.yaml',
    'k8s/grafana.yaml',
    'k8s/jaeger.yaml',
    'k8s/otel-collector.yaml',
])

# Load petstore application (depends on data stores and observability)
k8s_yaml([
    'k8s/petstore-deployment.yaml',
    'k8s/petstore-service.yaml',
])

# ============================================================================
# RESOURCE CONFIGURATION
# ============================================================================

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

# Observability stack - depends on data stores
k8s_resource(
    'prometheus',
    port_forwards=['9090:9090'],
    resource_deps=['postgres', 'redis'],
    labels=['observability'],
)

k8s_resource(
    'grafana',
    port_forwards=['3000:3000'],
    resource_deps=['prometheus'],
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
    resource_deps=['jaeger', 'prometheus'],
    labels=['observability'],
)

# Petstore application - depends on everything
k8s_resource(
    'petstore',
    port_forwards=['8080:8080'],
    resource_deps=[
        'docker-build-and-load',  # Image must be built and loaded FIRST
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

# Button to run Goose load test
local_resource(
    'run-goose-test',
    'cargo run --release --example api_load_test -- --host http://localhost:8080 --users 10 --hatch-rate 2 --run-time 30s',
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

  📦 Pet Store API:    http://localhost:8080 (standard HTTP port)
  📊 Grafana:          http://localhost:3000 (admin/admin)
  📈 Prometheus:       http://localhost:9090 (standard Prometheus port)
  🔍 Jaeger UI:        http://localhost:16686
  🎛️  Tilt Dashboard:   http://localhost:{tilt_port} (press 'space' to open)
  
  🗄️  PostgreSQL:       localhost:5432 (user: brrtrouter, db: brrtrouter)
  🔴 Redis:            localhost:6379 (exposed for external tools)""".format(tilt_port=tilt_port) + """

🏗️  Startup Order:
  1. PostgreSQL & Redis (data stores)
  2. Prometheus, Grafana, Jaeger, OTEL Collector (observability)
  3. Pet Store API (application)

🔧 Quick Actions (in Tilt UI):
  - Click "regenerate-petstore" to rebuild from OpenAPI spec
  - Click "run-curl-tests" to test all endpoints
  - Click "run-goose-test" to run load test

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

