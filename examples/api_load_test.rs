//! Standalone Goose load test binary for BRRTRouter API
//!
//! Tests ALL OpenAPI endpoints under load to find memory leaks and performance issues.
//!
//! # Usage
//!
//! ```bash
//! # Run against local server
//! cargo run --release --example api_load_test -- \
//!   --host http://localhost:8080 \
//!   --users 50 \
//!   --hatch-rate 10 \
//!   --run-time 5m \
//!   --no-reset-metrics \
//!   --report-file load-test-report.html
//!
//! # Short test for CI
//! cargo run --release --example api_load_test -- \
//!   --host http://localhost:8080 \
//!   -u10 -r2 -t30s \
//!   --no-reset-metrics
//! ```
//!
//! # Authentication
//!
//! Authenticated endpoints (pets, users) automatically include the X-API-Key header.
//! The API key is hardcoded in the test functions for simplicity.

use goose::prelude::*;

/// Test GET /health endpoint (built-in, no auth required)
async fn test_health(user: &mut GooseUser) -> TransactionResult {
    user.get("health")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

/// Test GET /metrics endpoint (built-in, no auth required)
async fn test_metrics(user: &mut GooseUser) -> TransactionResult {
    user.get("metrics")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

/// Test GET /pets endpoint (authenticated)
async fn test_list_pets(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user.get_request_builder(&GooseMethod::Get, "pets")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /pets/{id} endpoint (authenticated)
async fn test_get_pet(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user.get_request_builder(&GooseMethod::Get, "pets/12345")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /users endpoint (authenticated)
async fn test_list_users(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user.get_request_builder(&GooseMethod::Get, "users")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

/// Test GET /users/{id} endpoint (authenticated)
async fn test_get_user(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user.get_request_builder(&GooseMethod::Get, "users/12345")?
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
    user.get("docs")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

/// Test static file: CSS
async fn test_static_css(user: &mut GooseUser) -> TransactionResult {
    user.get("css/styles.css")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

/// Test static file: JavaScript
async fn test_static_js(user: &mut GooseUser) -> TransactionResult {
    user.get("js/app.js")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    GooseAttack::initialize()?
        // Built-in endpoints (20% weight) - no auth required
        .register_scenario(
            scenario!("Built-in Endpoints")
                .set_weight(20)?
                .register_transaction(
                    transaction!(test_health)
                        .set_name("GET /health")
                )
                .register_transaction(
                    transaction!(test_metrics)
                        .set_name("GET /metrics")
                )
        )
        // Pet API (30% weight) - requires API key
        .register_scenario(
            scenario!("Pet API")
                .set_weight(30)?
                .register_transaction(
                    transaction!(test_list_pets)
                        .set_name("GET /pets (with auth)")
                )
                .register_transaction(
                    transaction!(test_get_pet)
                        .set_name("GET /pets/{id} (with auth)")
                )
        )
        // User API (30% weight) - requires API key
        .register_scenario(
            scenario!("User API")
                .set_weight(30)?
                .register_transaction(
                    transaction!(test_list_users)
                        .set_name("GET /users (with auth)")
                )
                .register_transaction(
                    transaction!(test_get_user)
                        .set_name("GET /users/{id} (with auth)")
                )
        )
        // Static Files (15% weight)
        .register_scenario(
            scenario!("Static Files")
                .set_weight(15)?
                .register_transaction(
                    transaction!(test_openapi_spec)
                        .set_name("GET /openapi.yaml")
                )
                .register_transaction(
                    transaction!(test_swagger_ui)
                        .set_name("GET /docs (Swagger UI)")
                )
        )
        // Static Assets (5% weight)
        .register_scenario(
            scenario!("Static Assets")
                .set_weight(5)?
                .register_transaction(
                    transaction!(test_static_css)
                        .set_name("GET /css/styles.css")
                )
                .register_transaction(
                    transaction!(test_static_js)
                        .set_name("GET /js/app.js")
                )
        )
        .execute()
        .await?;
    
    Ok(())
}

