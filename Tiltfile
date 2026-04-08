# BRRTRouter Local Development with Tilt
# Fast iteration with local builds + minimal container updates

# Set minimum Tilt version
version_settings(constraint='>=0.33.0')

# Default cluster name `kind` → kubectl context `kind-kind` (see microscaler/shared-kind-cluster).
# Legacy dedicated cluster context (older docs/scripts).
allow_k8s_contexts(['kind-kind', 'kind-brrtrouter-dev'])

# Tilt web UI: use **10353** to avoid 10350 (default Tilt / other stacks). Pass on the CLI:
#   tilt up --port 10353
# (`just dev-up` does this.) Setting TILT_PORT only in Starlark is too late — Tilt binds before Tiltfile runs.
update_settings(k8s_upsert_timeout_secs=60)
config.define_string('tilt_port', args=False, usage='Port for Tilt web UI (must match tilt up --port)')
cfg = config.parse()
tilt_port = cfg.get('tilt_port', '10353')

# Keep in sync with justfile `tilt up --port …` for docs/printouts only
os.putenv('TILT_PORT', tilt_port)

# Skip Prometheus, Grafana, Loki, Jaeger, OTEL Collector, Pyroscope (faster `tilt ci` / kind ready).
# Set TILT_SKIP_OBSERVABILITY=1 (or true/yes). Pet Store still references otel-collector in YAML;
# OTLP export may log connection errors until a collector exists — acceptable for CI smoke tests.
skip_observability = os.environ.get('TILT_SKIP_OBSERVABILITY', '').lower() in ('1', 'true', 'yes')

# Local dev: use Postgres/Redis/observability from microscaler/shared-kind-cluster (namespaces data, observability)
# instead of applying k8s/data and k8s/observability from this repo. CI (CI=true) always bundles manifests for a standalone clone.
# Override: TILT_USE_SHARED_KIND_INFRA=0 to deploy bundled postgres/redis/observability in brrtrouter-dev (legacy).
is_ci = os.environ.get('CI', 'false').lower() == 'true'
_explicit_shared = os.environ.get('TILT_USE_SHARED_KIND_INFRA', '').strip().lower()
if _explicit_shared in ('0', 'false', 'no'):
    use_shared_kind_infra = False
elif _explicit_shared in ('1', 'true', 'yes'):
    use_shared_kind_infra = True
else:
    use_shared_kind_infra = not is_ci

bundled_postgres_redis = not use_shared_kind_infra
bundled_observability = bundled_postgres_redis and (not skip_observability)

if use_shared_kind_infra:
    print('ℹ️  Shared cluster infra: using Postgres/Redis in namespace data and OTEL/metrics in observability (shared-kind-cluster).')
elif skip_observability:
    print('ℹ️  Observability stack skipped (TILT_SKIP_OBSERVABILITY). Postgres, Redis, and app still deploy.')

# Host-aware build selection via shell script (exception approved)
brr_build_cmd = 'scripts/host-aware-build.sh brr'
pet_build_cmd = 'scripts/host-aware-build.sh pet'

# ============================================================================
# LOCAL BUILDS (Fast incremental compilation on host)
# ============================================================================

# Python package: brrtrouter-tooling (CLI `brrtrouter`, workspace helpers, MCP, etc.)
# Mirrors `just init` / `just build-tooling`: editable install into tooling/.venv
local_resource(
    'build-brrtrouter-tooling',
    '''
set -euo pipefail
if [ ! -d tooling/.venv ]; then
  python3 -m venv tooling/.venv
  tooling/.venv/bin/pip install --upgrade pip
fi
tooling/.venv/bin/pip install -e "./tooling[dev]"
''',
    deps=[
        'tooling/pyproject.toml',
        'tooling/src',
    ],
    ignore=[
        'tooling/.venv',
        'tooling/build',
        'tooling/**/__pycache__',
        'tooling/src/brrtrouter_tooling.egg-info',
    ],
    labels=['build'],
    allow_parallel=True,
)

# 0. Build sample-ui (SolidJS + Tailwind)
# Builds the rich dashboard UI and outputs to examples/pet_store/static_site
local_resource(
    'build-sample-ui',
    'cd sample-ui && yarn install --frozen-lockfile && yarn build:petstore',
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
    resource_deps=([] if (use_shared_kind_infra or skip_observability) else ['prometheus', 'loki', 'promtail']),
    deps=['src/', 'Cargo.toml', 'Cargo.lock'],
    labels=['build'],
    allow_parallel=True,
)

# 2. Generate pet_store from OpenAPI spec locally
# Use the built debug binary directly for speed (instant vs minutes for cargo run)
local_resource(
    'gen-petstore',
    './target/debug/brrtrouter-gen generate --spec examples/openapi.yaml --output examples/pet_store --force || cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --output examples/pet_store --force',
    deps=['examples/openapi.yaml', 'templates/', 'src/generator/', 'sample-ui/'],
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
# Build petstore: cross-compiled for Docker (Linux x86_64 musl)
# Docker containers need Linux binaries, so we always cross-compile for containerized deployment.
# For local native testing, use: SKIP_CROSS_COMPILE=1 scripts/host-aware-build.sh pet
local_resource(
    'build-petstore',
    pet_build_cmd + ' && mkdir -p build_artifacts && cp target/x86_64-unknown-linux-musl/debug/pet_store build_artifacts/',
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
# Load Data Stores (PostgreSQL, Redis) — only when not using shared-kind-cluster
# ============================================================================
if bundled_postgres_redis:
    k8s_yaml([
        'k8s/data/postgres.yaml',
        'k8s/data/redis.yaml',
    ])

# ============================================================================
# Load Observability Stack — only when bundling (not shared) and not skipped (e.g. CI smoke)
# ============================================================================
if bundled_observability:
    k8s_yaml('k8s/observability/storage.yaml')
    k8s_yaml([
        'k8s/observability/prometheus.yaml',
        'k8s/observability/loki.yaml',
        'k8s/observability/promtail.yaml',
        'k8s/observability/grafana.yaml',
        'k8s/observability/grafana-dashboard.yaml',
        'k8s/observability/jaeger.yaml',
        'k8s/observability/otel-collector.yaml',
        'k8s/observability/pyroscope.yaml',
    ])

# ============================================================================
# Load Application (Pet Store) - Using Kustomize overlays
# ============================================================================
# - CI: ci overlay (bundled infra in this namespace)
# - Local + shared cluster: shared overlay (FQDNs to data / observability namespaces)
# - Local + bundled: local overlay (debug) with postgres/redis from k8s/data
if is_ci:
    app_overlay = 'k8s/app/overlays/ci'
    _overlay_note = ' (CI)'
elif use_shared_kind_infra:
    app_overlay = 'k8s/app/overlays/shared'
    _overlay_note = ' (shared-kind-cluster endpoints)'
else:
    app_overlay = 'k8s/app/overlays/local'
    _overlay_note = ' (bundled postgres/redis in brrtrouter-dev)'

print('📦 Using kustomize overlay: ' + app_overlay + _overlay_note)
k8s_yaml(kustomize(app_overlay))

# ============================================================================
# RESOURCE CONFIGURATION
# ============================================================================

# Velero backup system - independent, starts early (only if enabled)
if velero_enabled:
    k8s_resource(
        'velero',
        labels=['backup'],
    )

# Data stores (only when manifests are loaded above)
if bundled_postgres_redis:
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

# Observability (only when bundled manifests are loaded)
# Note: Port forwarding handled by Kind NodePort mappings (see kind-config.yaml)
# NodePorts: Prometheus 31090, Grafana 31300, Jaeger 31166, Loki 31310, OTEL 31417/31418/31889, Pyroscope 31404
# Host ports: 9090, 3000, 16686, 3100, 4317/4318/8889, 4040
if bundled_observability:
    k8s_resource(
        'prometheus',
        labels=['observability'],
    )

    k8s_resource(
        'loki',
        resource_deps=['prometheus'],
        labels=['observability'],
    )

    k8s_resource(
        'promtail',
        resource_deps=['loki'],
        labels=['observability'],
    )

    k8s_resource(
        'grafana',
        resource_deps=['prometheus', 'loki', 'jaeger'],
        labels=['observability'],
    )

    k8s_resource(
        'jaeger',
        resource_deps=['postgres', 'redis'],
        labels=['observability'],
    )

    k8s_resource(
        'otel-collector',
        resource_deps=['jaeger', 'prometheus', 'loki'],
        labels=['observability'],
    )

    k8s_resource(
        'pyroscope',
        labels=['observability'],
    )

# Petstore application - depends on data + image when bundled; otherwise image only
# Port forwarding: Tilt port forward 8080:8080 (also available via Kind NodePort 31080 -> host 8080)
petstore_deps = ['docker-build-and-push']
if bundled_postgres_redis:
    petstore_deps += ['postgres', 'redis']
if bundled_observability:
    petstore_deps += ['prometheus', 'otel-collector']

k8s_resource(
    'petstore',
    port_forwards='8080:8080',
    resource_deps=petstore_deps,
    labels=['app'],
    # Auto-reconnect on pod restart
    auto_init=True,
    trigger_mode=TRIGGER_MODE_AUTO,
)

# ============================================================================
# HELPFUL COMMANDS
# ============================================================================

# Button to manually regenerate petstore code
# Use the built debug binary directly for speed (instant vs minutes for cargo run)
local_resource(
    'regenerate-petstore',
    './target/debug/brrtrouter-gen generate --spec examples/openapi.yaml --output examples/pet_store --force || cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --output examples/pet_store --force',
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
    'cargo run --example api_load_test -- --host http://localhost:8080 --users 25 --increase-rate 2 --run-time 300s',
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
    'cargo run --example adaptive_load_test -- --host http://localhost:8080',
    resource_deps=['petstore'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    labels=['tools'],
    auto_init=False,
)

# ============================================================================
# DISPLAY INFORMATION
# ============================================================================

_obs_urls = '' if (skip_observability and not use_shared_kind_infra) else """
  📊 Grafana:          http://localhost:3000 (admin/admin - includes Loki/Jaeger datasources)
  📈 Prometheus:       http://localhost:9090
  📋 Loki (logs):      http://localhost:3100
  🔍 Jaeger UI:        http://localhost:16686
"""
_obs_order = (
    '  2. Pet Store API (application)\n'
    if skip_observability and not use_shared_kind_infra else
    (
        '  2. Pet Store API (application) — Postgres/Redis/OTEL from shared-kind-cluster\n'
        if use_shared_kind_infra else
        (
            '  2. Prometheus, Loki + Promtail, Grafana, Jaeger, OTEL Collector (observability)\n'
            + '  3. Pet Store API (application)\n'
        )
    )
)
_data_redis_line = (
    '  🗄️  PostgreSQL / Redis: namespaces `data` + `observability` (shared-kind-cluster; same kind port maps)\n'
    if use_shared_kind_infra else
    '  🗄️  PostgreSQL:       localhost:5432 (user: brrtrouter, db: brrtrouter)\n  🔴 Redis:            localhost:6379'
)
print("""
╔══════════════════════════════════════════════════════════════════════╗
║                 BRRTRouter Local Development                         ║
╚══════════════════════════════════════════════════════════════════════╝

🚀 Services will be available at:

  📦 Pet Store API:    http://localhost:8080
{obs_urls}  🎛️  Tilt Dashboard:   http://localhost:{tilt_port} (press 'space' to open)
  
{data_redis_line}""".format(tilt_port=tilt_port, obs_urls=_obs_urls, data_redis_line=_data_redis_line) + """

🏗️  Startup Order:
  1. """ + ('Shared cluster (already running)' if use_shared_kind_infra else 'PostgreSQL & Redis (data stores)') + """
{obs_order}""".format(obs_order=_obs_order) + """
🔧 Quick Actions (in Tilt UI):
  - "build-brrtrouter-tooling" — pip install -e tooling[dev] (Python CLI / workspace tools)
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
  - """ + ('Postgres/Redis: ensure shared-kind-cluster is up (namespaces data + observability).' if use_shared_kind_infra else 'PostgreSQL and Redis are ready for controllers to use.') + """

════════════════════════════════════════════════════════════════════════
""")

