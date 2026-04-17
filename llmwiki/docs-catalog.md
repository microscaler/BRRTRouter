# Docs Catalog (Initial Ingest)

- Status: partially-verified
- Scope: full `docs/**/*.md` inventory (267 files)

## Inventory by major area

| Area | Count | Notes |
|---|---:|---|
| `docs/wip` | 154 | Historical and work-in-progress material; high staleness risk |
| `docs/EPICS` | 42 | Planning/spec material; partial implementation overlap |
| `docs/SPIFFY_mTLS` | 10 | Security architecture/design docs |
| `docs/tasks` | 7 | Task and PRD planning docs |
| `docs/JSF` + `docs/JSF_COMPLIANCE.md` | 7 | Safety/performance discipline docs |
| `docs/ADRS` | 2 | Architectural decision records |
| root-level docs in `docs/` | 45 | Mixed operational, architecture, performance, and audits |

## Full file inventory

- `docs/ADRS/001-async-generators-and-finding-an-alternative.md`
- `docs/ADRS/002-BFF-Implementations-for-microservices.md`
- `docs/ARCHITECTURE.md`
- `docs/BFF_PROXY_ANALYSIS.md`
- `docs/BRRTRouter_OVERVIEW.md`
- `docs/BRRTRouter_blog.md`
- `docs/CORS.md`
- `docs/CORS_IMPLEMENTATION_AUDIT.md`
- `docs/CORS_OPERATIONS.md`
- `docs/DECLARATIVE_INFRASTRUCTURE_AUDIT.md`
- `docs/DEPENDENCIES_CONFIG_GUIDE.md`
- `docs/DEPENDENCY_CONFIG_OPTIONS.md`
- `docs/DEPENDENCY_REGISTRY_USAGE.md`
- `docs/DEPENDENCY_SYSTEM_SUMMARY.md`
- `docs/DESIGN_ANALYSIS.md`
- `docs/DEVELOPMENT.md`
- `docs/EPICS/BFF_PROXY/BFF_GENERATOR_EXTRACTION_ANALYSIS.md`
- `docs/EPICS/BFF_PROXY/EPICS_AND_STORIES_SUMMARY.md`
- `docs/EPICS/BFF_PROXY/README.md`
- `docs/EPICS/BFF_PROXY/WIKI_LANDING_PAGE.md`
- `docs/EPICS/BFF_PROXY/WIKI_STRUCTURE.md`
- `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/README.md`
- `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.1-route-meta-extensions.md`
- `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.2-bff-generator-proxy-extensions.md`
- `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.3-bff-generator-components-security.md`
- `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.4-extract-bff-tooling-to-brrrouter.md`
- `docs/EPICS/BFF_PROXY/epic-2-proxy-library/README.md`
- `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.1-proxy-library.md`
- `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.2-downstream-base-url-config.md`
- `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.3-askama-proxy-handler.md`
- `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.4-bff-proxy-integration.md`
- `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/README.md`
- `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/story-3.1-bff-openapi-security-schemes.md`
- `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/story-3.2-optional-claims-enrichment.md`
- `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/story-3.3-rbac-from-jwt-or-idam.md`
- `docs/EPICS/BFF_PROXY/epic-4-enrich-downstream/README.md`
- `docs/EPICS/BFF_PROXY/epic-4-enrich-downstream/story-4.1-proxy-claim-headers.md`
- `docs/EPICS/BFF_PROXY/epic-4-enrich-downstream/story-4.2-configurable-claim-header-mapping.md`
- `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/README.md`
- `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/story-5.1-expose-jwt-claims-typed-handlers.md`
- `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/story-5.2-lifeguard-session-claims.md`
- `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/story-5.3-microservice-auth-model.md`
- `docs/EPICS/EPICS_CATALOG.md`
- `docs/EPICS/IDAM/EPICS_AND_STORIES_SUMMARY.md`
- `docs/EPICS/IDAM/README.md`
- `docs/EPICS/IDAM/epic-6-idam-contract/README.md`
- `docs/EPICS/IDAM/epic-6-idam-contract/story-6.1-document-idam-contract.md`
- `docs/EPICS/IDAM/epic-6-idam-contract/story-6.2-reference-idam-core-openapi.md`
- `docs/EPICS/IDAM/epic-7-idam-core/README.md`
- `docs/EPICS/IDAM/epic-7-idam-core/story-7.1-idam-core-service-skeleton.md`
- `docs/EPICS/IDAM/epic-7-idam-core/story-7.2-gotrue-client-integration.md`
- `docs/EPICS/IDAM/epic-7-idam-core/story-7.3-optional-session-redis.md`
- `docs/EPICS/IDAM/epic-8-idam-extension/README.md`
- `docs/EPICS/IDAM/epic-8-idam-extension/story-8.1-core-extension-spec-merge.md`
- `docs/EPICS/IDAM/epic-8-idam-extension/story-8.2-path-conventions-ingress.md`
- `docs/EPICS/IDAM/epic-9-bff-idam/README.md`
- `docs/EPICS/IDAM/epic-9-bff-idam/story-9.1-bff-idam-base-url-config.md`
- `docs/EPICS/IDAM/epic-9-bff-idam/story-9.2-document-bff-usage-idam.md`
- `docs/GENERATOR_IMPL_AND_DEPENDENCIES_ANALYSIS.md`
- `docs/GOOSE_LOAD_TESTING.md`
- `docs/HELPERS_AUDIT.md`
- `docs/IDAM_DESIGN_CORE_AND_EXTENSION.md`
- `docs/IDAM_GOTRUE_API_MAPPING.md`
- `docs/IDAM_MICROSCALER_ANALYSIS.md`
- `docs/JSF/DOGFOOD_PERFORMANCE_REGRESSION_ANALYSIS.md`
- `docs/JSF/DogfoodGooseMetrics.md`
- `docs/JSF/JSF_AUDIT_OPINION.md`
- `docs/JSF/JSF_WRITEUP.md`
- `docs/JSF/MainBranchGooseMetrics.md`
- `docs/JSF/PERFORMANCE_OPTIMIZATION_PRD.md`
- `docs/JSF_COMPLIANCE.md`
- `docs/K8S_DIRECTORY_STRUCTURE.md`
- `docs/KIND_TROUBLESHOOTING.md`
- `docs/LOAD_TESTING_GUIDE.md`
- `docs/LOCAL_DEVELOPMENT.md`
- `docs/LogAnalysis.md`
- `docs/MIGRATION_TYPED_HANDLER_HTTP_STATUS.md`
- `docs/PERFORMANCE.md`
- `docs/PERFORMANCE_ANALYSIS.md`
- `docs/PERFORMANCE_METRICS.md`
- `docs/PRD_TYPED_HANDLER_HTTP_STATUS.md`
- `docs/PUBLISHING.md`
- `docs/README.md`
- `docs/ROADMAP.md`
- `docs/RequestLifecycle.md`
- `docs/SPIFFY_mTLS/01_High-Assurance Multi-Tenant Identity and Access Control Architecture-Part1.md`
- `docs/SPIFFY_mTLS/02_High-Security Multi-Tenant Auth & AuthZ Architecture-Part2.md`
- `docs/SPIFFY_mTLS/03_Database-Level Authorization with Supabase_ RLS and Multi-Tenant Security-Part3.md`
- `docs/SPIFFY_mTLS/04_Design Plan_ SPIFFE-Based mTLS for BRRTRouter Services.md`
- `docs/SPIFFY_mTLS/Generic_Access_Management_Service_Design.md`
- `docs/SPIFFY_mTLS/Generic_Identity_Service_IDAM_Design.md`
- `docs/SPIFFY_mTLS/IDAM_OpenAPI_and_Integration.md`
- `docs/SPIFFY_mTLS/PRD_SPIFFE_mTLS_Multi-Tenant_Security.md`
- `docs/SPIFFY_mTLS/SPIFFE_SPIRE Mutual TLS Architecture for BRRTRouter Services.md`
- `docs/SPIFFY_mTLS/Sesame_IDAM_Audit_and_Transformation_Analysis.md`
- `docs/SecurityAuthentication.md`
- `docs/TEST_DOCUMENTATION.md`
- `docs/TILT_IMPLEMENTATION.md`
- `docs/TOOLING_AUDIT_RERP.md`
- `docs/VELERO_BACKUPS.md`
- `docs/VELERO_BACKUP_SYSTEM.md`
- `docs/VELERO_DECLARATIVE_SETUP.md`
- `docs/bugs.md`
- `docs/flamegraph.md`
- `docs/generatorOptions.md`
- `docs/stack_size.md`
- `docs/tasks/README.md`
- `docs/tasks/auth_issues_prd.md`
- `docs/tasks/code-quality-foundation-prd.md`
- `docs/tasks/development-workflow.md`
- `docs/tasks/flakey-tests-analysis.md`
- `docs/tasks/generatorImprovementsPRD.md`
- `docs/tasks/tasks.md`
- `docs/wip/AGENTS.md`
- `docs/wip/AUTO_BUILD_SOLUTION.md`
- `docs/wip/AddressBottlenecks.md`
- `docs/wip/BROKEN_PIPE_ERROR_ANALYSIS.md`
- `docs/wip/BROKEN_PIPE_FIX.md`
- `docs/wip/BRRTRouter_BLOG_OUTLINE.md`
- `docs/wip/BRRTRouter_Whitepaper.md`
- `docs/wip/BUILD_SIMPLIFICATION.md`
- `docs/wip/BottleNecksSecondOpinion.md`
- `docs/wip/CI_CLEANUP_FIX.md`
- `docs/wip/CI_ZIGBUILD_COMPLETE.md`
- `docs/wip/CI_ZIGBUILD_FIX.md`
- `docs/wip/CLIPPY_FIXES_SUMMARY.md`
- `docs/wip/COMPLEX_FUNCTIONS_DOCUMENTED.md`
- `docs/wip/COMPREHENSIVE_LOGGING_COMPLETE.md`
- `docs/wip/CONFIGMAP_APPROACH.md`
- `docs/wip/CONFIG_OVERRIDE_FIX.md`
- `docs/wip/CONNECTION_TELEMETRY.md`
- `docs/wip/CONTRIBUTOR_ONBOARDING.md`
- `docs/wip/CORS_AUDIT.md`
- `docs/wip/CORS_COMPLETION_SUMMARY.md`
- `docs/wip/CORS_CREDENTIALS_AUTH.md`
- `docs/wip/CORS_JWKS_SPIFFE_HOLISTIC_AUDIT.md`
- `docs/wip/CRASH_FIX_SUMMARY.md`
- `docs/wip/CROSS_COMPILE_FIX.md`
- `docs/wip/CURL_TESTS_COMPLETE_FIX.md`
- `docs/wip/CURL_TESTS_DOCKER_IMAGE.md`
- `docs/wip/CURL_TESTS_FIX_COMPLETE.md`
- `docs/wip/CURL_TEST_RACE_CONDITION_FIX.md`
- `docs/wip/DANGLING_IMAGES_FIX.md`
- `docs/wip/DOCKER_CLEANUP_FIX.md`
- `docs/wip/DOCKER_IMAGE_SETUP.md`
- `docs/wip/DOCUMENTATION.md`
- `docs/wip/DOCUMENTATION_COMPLETE.md`
- `docs/wip/DOCUMENTATION_FINAL_SUMMARY.md`
- `docs/wip/DOCUMENTATION_PROGRESS.md`
- `docs/wip/DOCUMENTATION_ULTIMATE_SUMMARY.md`
- `docs/wip/FINAL_COMMIT_SUMMARY.md`
- `docs/wip/FINAL_RAII_SUMMARY.md`
- `docs/wip/FIXING_TOO_MANY_HEADERS.md`
- `docs/wip/FIX_PERMISSIONS_CLARIFICATION.md`
- `docs/wip/FORK_AND_PR_COMMANDS.md`
- `docs/wip/GENERATOR_RS_BUG_REPORT.md`
- `docs/wip/HOT_RELOAD_DEBUG.md`
- `docs/wip/HOT_RELOAD_RAII_FIX.md`
- `docs/wip/HOT_RELOAD_TEST_FIX.md`
- `docs/wip/IMAGE_CLEANUP.md`
- `docs/wip/IMPL_BLOCKS_DOCUMENTED.md`
- `docs/wip/INSTRUMENTATION_GUIDE.md`
- `docs/wip/IN_MEMORY_SPAN_TESTING.md`
- `docs/wip/JEMALLOC_SETUP.md`
- `docs/wip/JSF_HOT_PATH_AUDIT.md`
- `docs/wip/JWKS_TEST_COVERAGE_GAPS.md`
- `docs/wip/JWT_BENCHMARKS.md`
- `docs/wip/JWT_DESIGN_REVIEW.md`
- `docs/wip/JWT_IMPROVEMENTS_SUMMARY.md`
- `docs/wip/JWT_PERFORMANCE_ANALYSIS.md`
- `docs/wip/JWT_PERFORMANCE_RESULTS.md`
- `docs/wip/JWT_REMAINING_IMPROVEMENTS.md`
- `docs/wip/JWT_REMAINING_ITEMS_PLAN.md`
- `docs/wip/KIND_IMAGE_LOADING_FIX.md`
- `docs/wip/KIND_LOCAL_REGISTRY.md`
- `docs/wip/KIND_REGISTRY_FIX.md`
- `docs/wip/LOAD_TESTING.md`
- `docs/wip/LOAD_TESTING_SUCCESS.md`
- `docs/wip/LOCAL_REGISTRY_FIX.md`
- `docs/wip/LOCAL_REGISTRY_IMPLEMENTATION.md`
- `docs/wip/LOGGING_PRD.md`
- `docs/wip/MAY_MINIHTTP_ISSUE_18_FINDINGS.md`
- `docs/wip/MAY_MINIHTTP_PATCH_APPLIED.md`
- `docs/wip/MEMORY_AND_ASSETS_ANALYSIS.md`
- `docs/wip/MEMORY_LEAK_FIX.md`
- `docs/wip/MEMORY_LEAK_FIX_SUMMARY.md`
- `docs/wip/NEXTEST_CI_MIGRATION.md`
- `docs/wip/NEXTEST_NAMEDTEMPFILE_ISSUE.md`
- `docs/wip/OBSCTL_PROVEN_VERSIONS.md`
- `docs/wip/OBSERVABILITY_COMPLETE.md`
- `docs/wip/OBSERVABILITY_FINAL_VERSIONS.md`
- `docs/wip/OBSERVABILITY_PROVEN_SETUP.md`
- `docs/wip/OBSERVABILITY_SETUP.md`
- `docs/wip/OBSERVABILITY_SETUP_COMPLETE.md`
- `docs/wip/OBSERVABILITY_STACK_COMPLETE.md`
- `docs/wip/OTLP_VERSION_CONFLICT.md`
- `docs/wip/PERFORMANCE_DASHBOARD.md`
- `docs/wip/PERFORMANCE_MCP_SERVER_PRD.md`
- `docs/wip/PERSISTENT_DOCKER_VOLUMES.md`
- `docs/wip/PERSISTENT_OBSERVABILITY_STORAGE.md`
- `docs/wip/PORT_MAPPING_FIX.md`
- `docs/wip/PRAGMATIC_IMAGE_SOLUTION.md`
- `docs/wip/PRIVATE_FUNCTIONS_DOCUMENTED.md`
- `docs/wip/PROMETHEUS_WAL_FIX.md`
- `docs/wip/PUB_CRATE_DOCUMENTATION.md`
- `docs/wip/RAII_AUDIT_COMPLETE.md`
- `docs/wip/RAII_FIXES_COMPLETE.md`
- `docs/wip/README.md`
- `docs/wip/README_UPDATE_SUMMARY.md`
- `docs/wip/REGISTRY_PROXY_COMPLETE.md`
- `docs/wip/REGISTRY_PROXY_FIXED.md`
- `docs/wip/REGISTRY_PROXY_SETUP.md`
- `docs/wip/REQUEST_LOGGING_IMPLEMENTED.md`
- `docs/wip/RICH_UI_DASHBOARD.md`
- `docs/wip/RUST_WITH_STATEMENT.md`
- `docs/wip/SAFE_IMAGE_CLEANUP.md`
- `docs/wip/SAMPLE_UI_BUILD_FLOW.md`
- `docs/wip/SAMPLE_UI_READY.md`
- `docs/wip/SAMPLE_UI_SETUP.md`
- `docs/wip/SAMPLE_UI_TILT_INTEGRATION.md`
- `docs/wip/SCRIPTS_REMOVAL_COMPLETE.md`
- `docs/wip/SCRIPTS_TO_JUSTFILE_MIGRATION.md`
- `docs/wip/SESSION_SUMMARY.md`
- `docs/wip/SESSION_SUMMARY_IMAGE_CLEANUP.md`
- `docs/wip/SIGINT_CLEANUP_FIX.md`
- `docs/wip/SIGINT_FIX_SUMMARY.md`
- `docs/wip/SOLIDJS_SHOWCASE_SUMMARY.md`
- `docs/wip/SOLIDJS_UI_COMPLETE.md`
- `docs/wip/SOLIDJS_UI_FEATURES.md`
- `docs/wip/SOLIDJS_UI_INTEGRATION.md`
- `docs/wip/SPEC_TESTS_RAII_FIX.md`
- `docs/wip/SPIFFE_COMPLIANCE_ASSESSMENT.md`
- `docs/wip/SPIFFE_FAILING_TESTS_ANALYSIS.md`
- `docs/wip/SPIFFE_IMPLEMENTATION_PLAN.md`
- `docs/wip/SPIFFE_IMPLEMENTATION_STATUS.md`
- `docs/wip/SPIFFE_JWKS_TEST_CONTAINER_PLAN.md`
- `docs/wip/SPIFFE_MICROSERVICE_AUDIT.md`
- `docs/wip/SPIFFE_PHASE2_STATUS.md`
- `docs/wip/SPIFFE_ROADMAP_FINTECH.md`
- `docs/wip/SSE_TESTING_SUMMARY.md`
- `docs/wip/STACK_SIZE_FIX.md`
- `docs/wip/STAGING_AREA_PATTERN.md`
- `docs/wip/STATIC_HARNESS_CLEANUP_FIX.md`
- `docs/wip/TAILWIND_SETUP_COMPLETE.md`
- `docs/wip/TELEMETRY_GAPS_AND_IMPROVEMENTS.md`
- `docs/wip/TESTING_UI.md`
- `docs/wip/TESTING_WITHOUT_SOLIDJS.md`
- `docs/wip/TEST_HEADER_LIMITS.md`
- `docs/wip/TEST_SETUP_TEARDOWN.md`
- `docs/wip/THREE_FIXES_SUMMARY.md`
- `docs/wip/TILT_CI_INTEGRATION.md`
- `docs/wip/TILT_DAG_ANALYSIS.md`
- `docs/wip/TILT_DEPENDENCY_CHAIN.md`
- `docs/wip/TILT_DEPENDENCY_FIX.md`
- `docs/wip/TILT_FIX_APPLIED.md`
- `docs/wip/TILT_PORT_CONFIGURATION.md`
- `docs/wip/TILT_SUCCESS.md`
- `docs/wip/TOOMANYHEADERS_FIX.md`
- `docs/wip/TOOMANYHEADERS_FIX_SUMMARY.md`
- `docs/wip/TOO_MANY_HEADERS_INVESTIGATION.md`
- `docs/wip/UI_DEPLOYMENT_SUCCESS.md`
- `docs/wip/UPSTREAM_PR_PLAN.md`
- `docs/wip/UPSTREAM_PR_TRACKER.md`
- `docs/wip/VENDORING_MAY_MINIHTTP.md`
- `docs/wip/WARNINGS_FIX_PLAN.md`
- `docs/wip/ignored_tests_analysis.md`
- `docs/wip/pipelineControlledReconcilliationGate.md`

## LLM wiki synthesis index (2026-04-17)

| Wiki page | Maps to |
|-----------|---------|
| [`topics/runtime-stack-map.md`](./topics/runtime-stack-map.md) | `spec/` → `router/` → `dispatcher/` → `server/service.rs` |
| [`topics/schema-validation-pipeline.md`](./topics/schema-validation-pipeline.md) | V1a/V1/V2/V6/V7 validation gates |
| [`topics/generator-cli-and-askama.md`](./topics/generator-cli-and-askama.md) | `brrtrouter_gen`, `src/generator/`, `templates/*.txt` |
| [`topics/sibling-repos-and-wikis.md`](./topics/sibling-repos-and-wikis.md) | Lifeguard + Hauliage wikis and responsibility split |
| [`entities/route-meta.md`](./entities/route-meta.md) | `RouteMeta` fields including `request_content_types` |
| [`entities/request-body-parsing.md`](./entities/request-body-parsing.md) | Content-Type parsing, multipart history |
| [`reference/openapi-extensions.md`](./reference/openapi-extensions.md) | All supported `x-*` keys |
| [`reference/codebase-entry-points.md`](./reference/codebase-entry-points.md) | File-level entry points |
| [`topics/canonical-docs-vs-wip.md`](./topics/canonical-docs-vs-wip.md) | Staleness policy for `docs/wip/` |
