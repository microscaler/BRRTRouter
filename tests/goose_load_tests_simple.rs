//! Simple Goose load test example for BRRTRouter
//!
//! This is a minimal working example showing how to load test
//! BRRTRouter's static file serving and API endpoints with Goose 0.18.1.
//!
//! Based on: https://docs.rs/goose/0.18.1/goose/
//!
//! Key Goose 0.18.1 features used:
//! - Scenario and Transaction (renamed from TaskSet/Task)
//! - GooseUser with async transactions
//! - TransactionResult for error handling
//! - Simplified prelude imports

#![cfg(test)]
#![allow(dead_code)]

use goose::prelude::*;

/// Load static CSS file
async fn load_css(user: &mut GooseUser) -> TransactionResult {
    let _goose = user.get("css/styles.css").await?;
    Ok(())
}

/// Load static JavaScript
async fn load_js(user: &mut GooseUser) -> TransactionResult {
    let _goose = user.get("js/app.js").await?;
    Ok(())
}

/// Load static JSON
async fn load_json(user: &mut GooseUser) -> TransactionResult {
    let _goose = user.get("api-info.json").await?;
    Ok(())
}

/// Check health endpoint (built-in to BRRTRouter)
async fn health_check(user: &mut GooseUser) -> TransactionResult {
    let _goose = user.get("health").await?;
    Ok(())
}

/// Simple load test configuration
///
/// # Usage
///
/// Run from command line as a standalone binary:
///
/// ```bash
/// # Create a new binary crate for load testing
/// cargo new --bin brrtrouter-loadtest
/// cd brrtrouter-loadtest
///
/// # Add dependencies to Cargo.toml:
/// # [dependencies]
/// # goose = "0.18.1"
/// # tokio = { version = "1", features = ["full"] }
///
/// # Copy this test code to src/main.rs and modify the main function to:
/// # #[tokio::main]
/// # async fn main() -> Result<(), GooseError> {
/// #     GooseAttack::initialize()?
/// #         .register_scenario(scenario!("Static Files")
/// #             .register_transaction(transaction!(load_css))
/// #             .register_transaction(transaction!(load_js))
/// #             .register_transaction(transaction!(load_json))
/// #         )
/// #         .register_scenario(scenario!("Health Check")
/// #             .register_transaction(transaction!(health_check))
/// #         )
/// #         .execute()
/// #         .await?;
/// #     Ok(())
/// # }
///
/// # Then run:
/// # cargo run -- --host http://localhost:8080 --users 10 --run-time 30s
/// ```
#[cfg(test)]
mod tests {
    #[test]
    fn test_goose_compiles() {
        // This test just verifies the Goose code compiles
        // Actual load testing should be done with a standalone binary or ignored test
        assert!(true, "Goose load test code compiles");
    }
}

