//! Standalone Goose load test binary for BRRTRouter API
//!
//! Tests ALL OpenAPI endpoints under load to find memory leaks and performance issues.
//!
//! **CLI:** Goose exposes per-second ramp as `--increase-rate` / `-r` (not `--hatch-rate`).
//! Run `cargo run --example api_load_test -- --help` for current flags.
//!
//! # Usage
//!
//! ```bash
//! # Run against local server
//! cargo run --release --example api_load_test -- \
//!   --host http://localhost:8081 \
//!   --users 50 \
//!   --increase-rate 10 \
//!   --run-time 5m \
//!   --no-reset-metrics \
//!   --report-file load-test-report.html
//!
//! # Short test for CI
//! cargo run --release --example api_load_test -- \
//!   --host http://localhost:8081 \
//!   -u10 -r2 -t30s \
//!   --no-reset-metrics
//! ```
//!
//! # Authentication
//!
//! Authenticated endpoints (pets, users) automatically include the X-API-Key header.
//! The API key is hardcoded in the test functions for simplicity.
//!
//! # Scope control (`BRRTR_BENCH_SCOPE`)
//!
//! By default this harness measures **only** endpoints that traverse
//! BRRTRouter's OpenAPI pipeline — that is, the full
//! `radix → param extract → dispatcher → handler coroutine → schema
//! validation → typed serde → response` path. It deliberately excludes
//! the three endpoints that `AppService` short-circuits before the
//! dispatcher (`/health`, `/metrics`, `/openapi.yaml`) and the `/`,
//! `/docs` root/docs paths whose only router work is an early radix
//! miss — these give inflated aggregate numbers because they never
//! exercise the schema / handler / serde stack that real traffic does.
//!
//! Set `BRRTR_BENCH_SCOPE=full` to enable the "Built-in Endpoints" and
//! "Static Files" scenarios as well (useful for smoke / CI
//! end-to-end sanity, not for perf numbers). Default is `openapi`.

use goose::prelude::*;

/// Bench scope: which scenarios to register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchScope {
    /// Only OpenAPI-dispatched endpoints (default). Excludes the three
    /// short-circuits (/health, /metrics, /openapi.yaml) and the
    /// radix-miss-only /, /docs paths.
    OpenApi,
    /// All scenarios, including built-ins and static files. For smoke
    /// testing; inflates aggregate req/s by ~15–20 % vs real workload.
    Full,
}

impl BenchScope {
    fn from_env() -> Self {
        match std::env::var("BRRTR_BENCH_SCOPE")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "full" | "all" => BenchScope::Full,
            _ => BenchScope::OpenApi,
        }
    }
}

/// Test GET /health endpoint (built-in, no auth required)
async fn test_health(user: &mut GooseUser) -> TransactionResult {
    user.get("health").await?.response?.error_for_status()?;
    Ok(())
}

/// Test GET /metrics endpoint (built-in, no auth required)
async fn test_metrics(user: &mut GooseUser) -> TransactionResult {
    user.get("metrics").await?.response?.error_for_status()?;
    Ok(())
}

/// Test GET /pets endpoint (authenticated)
async fn test_list_pets(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "pets")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /pets/{id} endpoint (authenticated)
async fn test_get_pet(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "pets/12345")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /users endpoint (authenticated)
async fn test_list_users(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "users")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /users/{id} endpoint (authenticated)
async fn test_get_user(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "users/abc-123")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test static file: OpenAPI spec
async fn test_openapi_spec(user: &mut GooseUser) -> TransactionResult {
    user.get("openapi.yaml")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

/// Test static file: Swagger UI
async fn test_swagger_ui(user: &mut GooseUser) -> TransactionResult {
    user.get("docs").await?.response?.error_for_status()?;
    Ok(())
}

/// Test POST /pets endpoint (authenticated)
async fn test_add_pet(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Post, "pets")?
        .header("X-API-Key", "test123")
        .header("Content-Type", "application/json")
        .body(r#"{"name":"Fluffy","species":"dog"}"#);
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /users/{user_id}/posts endpoint
async fn test_list_user_posts(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "users/abc-123/posts")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /users/{user_id}/posts/{post_id} endpoint
async fn test_get_user_post(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "users/abc-123/posts/post1")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /admin/settings endpoint
async fn test_admin_settings(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "admin/settings")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /items/{id} endpoint
async fn test_get_item(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(
            &GooseMethod::Get,
            "items/550e8400-e29b-41d4-a716-446655440000",
        )?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /search endpoint with query params
async fn test_search(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "search?q=test&category=all&limit=10")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /labels/{color} endpoint (label-style path parameters)
async fn test_label_path(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "labels/.red")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test static file: Root index page
async fn test_index(user: &mut GooseUser) -> TransactionResult {
    user.get("/").await?.response?.error_for_status()?;
    Ok(())
}

/// Test POST /items/{id} endpoint (create/update item)
async fn test_post_item(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(
            &GooseMethod::Post,
            "items/550e8400-e29b-41d4-a716-446655440000",
        )?
        .header("X-API-Key", "test123")
        .header("Content-Type", "application/json")
        .body(r#"{"name":"New Item"}"#);
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

// TODO: Add download endpoint back in when auth issues addressed
// /// Test GET /download/{id} endpoint
// async fn test_download(user: &mut GooseUser) -> TransactionResult {
//     let request_builder = user
//         .get_request_builder(&GooseMethod::Get, "download/550e8400-e29b-41d4-a716-446655440000")?
//         .header("X-API-Key", "test123");
//     let goose_request = GooseRequest::builder()
//         .set_request_builder(request_builder)
//         .build();
//     user.request(goose_request).await?;
//     Ok(())
// }

/// Test POST /webhooks endpoint
async fn test_webhook(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Post, "webhooks")?
        .header("X-API-Key", "test123")
        .header("Content-Type", "application/json")
        .body(r#"{"url":"https://example.com/webhook"}"#);
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test DELETE /users/{id} endpoint
async fn test_delete_user(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Delete, "users/abc-123")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    let scope = BenchScope::from_env();
    eprintln!("[bench] scope = {scope:?} (set BRRTR_BENCH_SCOPE=full to include short-circuits)");

    let mut attack = GooseAttack::initialize()?;

    // Built-in Endpoints (10 % weight in Full scope) — `/health`, `/metrics`
    // short-circuit in `AppService` before the dispatcher, so they never
    // exercise the radix → params → handler → schema → serde pipeline that
    // BRRTRouter is about. Only register in Full scope.
    if scope == BenchScope::Full {
        attack = attack.register_scenario(
            scenario!("Built-in Endpoints")
                .set_weight(10)?
                .register_transaction(transaction!(test_health).set_name("GET /health"))
                .register_transaction(transaction!(test_metrics).set_name("GET /metrics")),
        );
    }

    attack = attack
        // Pet API (25% weight) - requires API key
        .register_scenario(
            scenario!("Pet API")
                .set_weight(25)?
                .register_transaction(
                    transaction!(test_list_pets).set_name("GET /pets (with auth)"),
                )
                .register_transaction(
                    transaction!(test_get_pet).set_name("GET /pets/{id} (with auth)"),
                )
                .register_transaction(
                    transaction!(test_add_pet).set_name("POST /pets (with auth)"),
                ),
        )
        // User API (20% weight) - requires API key
        .register_scenario(
            scenario!("User API")
                .set_weight(20)?
                .register_transaction(
                    transaction!(test_list_users).set_name("GET /users (with auth)"),
                )
                .register_transaction(
                    transaction!(test_get_user).set_name("GET /users/{id} (with auth)"),
                )
                .register_transaction(
                    transaction!(test_list_user_posts)
                        .set_name("GET /users/{id}/posts (with auth)"),
                )
                .register_transaction(
                    transaction!(test_get_user_post)
                        .set_name("GET /users/{id}/posts/{post_id} (with auth)"),
                )
                .register_transaction(
                    transaction!(test_delete_user).set_name("DELETE /users/{id} (with auth)"),
                ),
        )
        // Advanced API (25% weight) - requires API key
        .register_scenario(
            scenario!("Advanced API")
                .set_weight(25)?
                .register_transaction(
                    transaction!(test_search).set_name("GET /search?q=test (with auth)"),
                )
                .register_transaction(
                    transaction!(test_get_item).set_name("GET /items/{id} (with auth)"),
                )
                .register_transaction(
                    transaction!(test_post_item).set_name("POST /items/{id} (with auth)"),
                )
                .register_transaction(
                    transaction!(test_admin_settings).set_name("GET /admin/settings (with auth)"),
                )
                // TODO: Add download endpoint back in when auth issues addressed
                // .register_transaction(
                //     transaction!(test_download).set_name("GET /download/{id} (with auth)"),
                // )
                .register_transaction(
                    transaction!(test_webhook).set_name("POST /webhooks (with auth)"),
                ),
        )
        // Path Parameters (10% weight) - requires API key
        .register_scenario(
            scenario!("Path Parameters")
                .set_weight(10)?
                .register_transaction(
                    transaction!(test_label_path).set_name("GET /labels/{color} (label-style)"),
                ),
        );

    // Static Files (10 % weight in Full scope) — `/openapi.yaml` is served
    // from disk via a dedicated short-circuit (not the dispatcher), and
    // `/`, `/docs` exit on the first radix miss as 404s without ever
    // reaching handler / schema / serde. Only register in Full scope.
    if scope == BenchScope::Full {
        attack = attack.register_scenario(
            scenario!("Static Files")
                .set_weight(10)?
                .register_transaction(transaction!(test_openapi_spec).set_name("GET /openapi.yaml"))
                .register_transaction(
                    transaction!(test_swagger_ui).set_name("GET /docs (Swagger UI)"),
                )
                .register_transaction(transaction!(test_index).set_name("GET / (root)")),
        );
    }

    attack.execute().await?;

    Ok(())
}
