//! Adaptive Goose load test that finds BRRTRouter's breaking point
//!
//! This test runs in a continuous loop, querying Prometheus after each cycle
//! to check error rates. It automatically increases load until the error rate
//! reaches the threshold (default: 5%), identifying the exact breaking point.
//!
//! # Usage
//!
//! ```bash
//! # Default: 5-minute cycles, ramp from 10 → 10,000 users
//! cargo run --release --example adaptive_load_test -- \
//!   --host http://localhost:8080
//!
//! # Quick test: 1-minute cycles, max 100 users
//! STAGE_DURATION=60 MAX_USERS=100 \
//! cargo run --release --example adaptive_load_test -- \
//!   --host http://localhost:8080
//!
//! # Custom configuration
//! START_USERS=20 RAMP_STEP=100 ERROR_RATE_THRESHOLD=3.0 \
//! cargo run --release --example adaptive_load_test -- \
//!   --host http://localhost:8080
//! ```
//!
//! # Environment Variables
//!
//! - `PROMETHEUS_URL`: Prometheus endpoint (default: http://localhost:9090)
//! - `START_USERS`: Starting user count (default: 10)
//! - `MAX_USERS`: Maximum user count safety limit (default: 10000)
//! - `RAMP_STEP`: Users to add per cycle (default: 50)
//! - `STAGE_DURATION`: Seconds per cycle (default: 300 = 5 minutes)
//! - `ERROR_RATE_THRESHOLD`: Max error rate % before stopping (default: 5.0)
//! - `P99_LATENCY_THRESHOLD`: P99 latency warning threshold in seconds (default: 2.0)
//! - `ACTIVE_REQUESTS_THRESHOLD`: Active requests warning threshold (default: 1000)

use goose::prelude::*;
use std::time::Duration;

// ============================================================================
// Goose Transaction Functions
// ============================================================================

async fn health_check(user: &mut GooseUser) -> TransactionResult {
    user.get("/health").await?.response?.error_for_status()?;
    Ok(())
}

async fn test_metrics(user: &mut GooseUser) -> TransactionResult {
    user.get("/metrics").await?.response?.error_for_status()?;
    Ok(())
}

async fn list_pets(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/pets")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn get_pet(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/pets/12345")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn search_pets(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/pets?name=fluffy")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn list_users(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/users")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn get_user(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/users/abc-123")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn load_openapi(user: &mut GooseUser) -> TransactionResult {
    user.get("/openapi.yaml")
        .await?
        .response?
        .error_for_status()?;
    Ok(())
}

async fn load_index(user: &mut GooseUser) -> TransactionResult {
    user.get("/").await?.response?.error_for_status()?;
    Ok(())
}

async fn add_pet(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Post, "/pets")?
        .header("X-API-Key", "test123")
        .header("Content-Type", "application/json")
        .body(r#"{"name":"Fluffy","species":"dog"}"#);
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn list_user_posts(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/users/abc-123/posts")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn get_user_post(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/users/abc-123/posts/post1")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn get_admin_settings(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/admin/settings")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn get_item(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(
            &GooseMethod::Get,
            "/items/550e8400-e29b-41d4-a716-446655440000",
        )?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn search_api(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/search?q=test&category=all&limit=10")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn label_path(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "/labels/.red")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn test_swagger_ui(user: &mut GooseUser) -> TransactionResult {
    user.get("/docs").await?.response?.error_for_status()?;
    Ok(())
}

async fn post_item(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(
            &GooseMethod::Post,
            "/items/550e8400-e29b-41d4-a716-446655440000",
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

async fn get_download(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(
            &GooseMethod::Get,
            "/download/550e8400-e29b-41d4-a716-446655440000",
        )?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn post_webhook(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Post, "/webhooks")?
        .header("X-API-Key", "test123")
        .header("Content-Type", "application/json")
        .body(r#"{"url":"https://example.com/webhook"}"#);
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

async fn delete_user(user: &mut GooseUser) -> TransactionResult {
    let request_builder = user
        .get_request_builder(&GooseMethod::Delete, "/users/abc-123")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();
    user.request(goose_request).await?;
    Ok(())
}

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Clone)]
struct AdaptiveConfig {
    host: String,
    prometheus_url: String,
    start_users: usize,
    max_users: usize,
    ramp_step: usize,
    hatch_rate: usize,
    stage_duration_secs: u64,
    error_rate_threshold: f64,
    p99_latency_threshold: f64,
    active_requests_threshold: f64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            host: std::env::var("GOOSE_HOST")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            prometheus_url: std::env::var("PROMETHEUS_URL")
                .unwrap_or_else(|_| "http://localhost:9090".to_string()),
            start_users: std::env::var("START_USERS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500), // Start low for better discovery
            max_users: std::env::var("MAX_USERS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(6000),
            ramp_step: std::env::var("RAMP_STEP")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            hatch_rate: std::env::var("HATCH_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50), // Users spawned per second - higher for faster ramp
            stage_duration_secs: std::env::var("STAGE_DURATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600), // Shorter 1-minute cycles for faster discovery
            error_rate_threshold: std::env::var("ERROR_RATE_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5.0),
            p99_latency_threshold: std::env::var("P99_LATENCY_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2.0),
            active_requests_threshold: std::env::var("ACTIVE_REQUESTS_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(25000.0),
        }
    }
}

// ============================================================================
// Prometheus Integration
// ============================================================================

#[derive(Debug)]
struct SystemMetrics {
    error_rate_percent: f64,
    p99_latency_seconds: f64,
    active_requests: f64,
    requests_per_second: f64,
}

async fn get_system_metrics(
    prometheus_url: &str,
) -> Result<SystemMetrics, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    // Query 1: Error rate (non-2xx responses / total responses * 100)
    // Use 30s window to focus on most recent data from the just-completed test
    let error_rate_query = urlencoding::encode(
        "100 * (sum(rate(brrtrouter_requests_total{status!~\"2..\"}[30s])) or vector(0)) / (sum(rate(brrtrouter_requests_total[30s])) or vector(1))"
    );
    let error_rate_url = format!("{}/api/v1/query?query={}", prometheus_url, error_rate_query);
    let error_rate_resp = client.get(&error_rate_url).send().await?;
    let error_rate_json: serde_json::Value = error_rate_resp.json().await?;
    let error_rate_percent = error_rate_json["data"]["result"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["value"][1].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Query 2: P99 latency
    let p99_query = urlencoding::encode(
        "histogram_quantile(0.99, rate(brrtrouter_request_duration_seconds_bucket[1m]))",
    );
    let p99_url = format!("{}/api/v1/query?query={}", prometheus_url, p99_query);
    let p99_resp = client.get(&p99_url).send().await?;
    let p99_json: serde_json::Value = p99_resp.json().await?;
    let p99_latency_seconds = p99_json["data"]["result"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["value"][1].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Query 3: Active requests (peak over 1m window)
    let active_query = urlencoding::encode("max_over_time(brrtrouter_active_requests[1m])");
    let active_url = format!("{}/api/v1/query?query={}", prometheus_url, active_query);
    let active_resp = client.get(&active_url).send().await?;
    let active_json: serde_json::Value = active_resp.json().await?;
    let active_requests = active_json["data"]["result"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["value"][1].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Query 4: Throughput (requests per second)
    let rps_query = urlencoding::encode("sum(rate(brrtrouter_requests_total[1m]))");
    let rps_url = format!("{}/api/v1/query?query={}", prometheus_url, rps_query);
    let rps_resp = client.get(&rps_url).send().await?;
    let rps_json: serde_json::Value = rps_resp.json().await?;
    let requests_per_second = rps_json["data"]["result"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["value"][1].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(SystemMetrics {
        error_rate_percent,
        p99_latency_seconds,
        active_requests,
        requests_per_second,
    })
}

// ============================================================================
// Main Adaptive Loop
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AdaptiveConfig::default();

    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║  BRRTRouter Adaptive Load Test 🎯                                        ║");
    println!("║  Continuous loop: incrementally increases load until failure             ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Target: {}", config.host);
    println!("  Prometheus: {}", config.prometheus_url);
    println!(
        "  Start Users: {} (increment: {} per cycle)",
        config.start_users, config.ramp_step
    );
    println!("  Max Users: {} (safety limit)", config.max_users);
    println!("  Hatch Rate: {} users/second", config.hatch_rate);
    println!(
        "  Cycle Duration: {}s per load level",
        config.stage_duration_secs
    );
    println!(
        "  Ramp-up Time: ~{}s to reach {} users",
        config.start_users / config.hatch_rate,
        config.start_users
    );
    println!();
    println!("Failure Threshold:");
    println!(
        "  🎯 Error Rate ≥ {:.1}% = LIMIT REACHED",
        config.error_rate_threshold
    );
    println!(
        "  ⚠️  P99 Latency > {:.2}s = WARNING",
        config.p99_latency_threshold
    );
    println!(
        "  ⚠️  Active Requests > {} = WARNING",
        config.active_requests_threshold
    );
    println!();
    println!(
        "Mode: CONTINUOUS - runs until error rate ≥ {:.1}% or max users reached",
        config.error_rate_threshold
    );
    println!("Press Ctrl+C to stop manually\n");

    let mut current_users = config.start_users;
    let mut cycle = 1;
    let mut last_healthy_users = config.start_users;
    let mut last_healthy_throughput = 0.0;

    loop {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!(
            "Cycle {} - Testing with {} concurrent users",
            cycle, current_users
        );
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Run Goose attack for this cycle
        GooseAttack::initialize()?
            .set_default(GooseDefault::Host, config.host.as_str())?
            .set_default(GooseDefault::Users, current_users)?
            .set_default(GooseDefault::RunTime, config.stage_duration_secs as usize)?
            .set_default(
                GooseDefault::HatchRate,
                config.hatch_rate.to_string().as_str(),
            )?
            .register_scenario(
                scenario!("Infrastructure")
                    .set_weight(10)?
                    .register_transaction(transaction!(health_check))
                    .register_transaction(transaction!(test_metrics)),
            )
            .register_scenario(
                scenario!("Pet Store API")
                    .set_weight(30)?
                    .register_transaction(transaction!(list_pets).set_weight(4)?)
                    .register_transaction(transaction!(get_pet).set_weight(5)?)
                    .register_transaction(transaction!(search_pets).set_weight(3)?)
                    .register_transaction(transaction!(add_pet).set_weight(2)?),
            )
            .register_scenario(
                scenario!("User API")
                    .set_weight(20)?
                    .register_transaction(transaction!(list_users).set_weight(3)?)
                    .register_transaction(transaction!(get_user).set_weight(4)?)
                    .register_transaction(transaction!(list_user_posts).set_weight(2)?)
                    .register_transaction(transaction!(get_user_post).set_weight(2)?)
                    .register_transaction(transaction!(delete_user).set_weight(1)?),
            )
            .register_scenario(
                scenario!("Advanced API")
                    .set_weight(20)?
                    .register_transaction(transaction!(search_api).set_weight(3)?)
                    .register_transaction(transaction!(get_item).set_weight(2)?)
                    .register_transaction(transaction!(post_item).set_weight(1)?)
                    .register_transaction(transaction!(get_admin_settings).set_weight(1)?)
                    .register_transaction(transaction!(get_download).set_weight(1)?)
                    .register_transaction(transaction!(post_webhook).set_weight(1)?),
            )
            .register_scenario(
                scenario!("Path Parameters")
                    .set_weight(10)?
                    .register_transaction(transaction!(label_path)),
            )
            .register_scenario(
                scenario!("Static Resources")
                    .set_weight(10)?
                    .register_transaction(transaction!(load_openapi))
                    .register_transaction(transaction!(load_index))
                    .register_transaction(transaction!(test_swagger_ui)),
            )
            .execute()
            .await?;

        // Transition to Prometheus health check
        println!("\n╭─────────────────────────────────────────────────────────────────────╮");
        println!(
            "│ 🔍 Checking Prometheus Metrics - Cycle {}                           │",
            cycle
        );
        println!("╰─────────────────────────────────────────────────────────────────────╯");
        println!("⏳ Waiting 5 seconds for Prometheus to scrape metrics...\n");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Query Prometheus for system health
        match get_system_metrics(&config.prometheus_url).await {
            Ok(system_metrics) => {
                println!("📊 System Metrics:");
                println!(
                    "  Error Rate: {:.2}% (threshold: {:.1}%)",
                    system_metrics.error_rate_percent, config.error_rate_threshold
                );
                println!(
                    "  P99 Latency: {:.3}s (threshold: {:.2}s)",
                    system_metrics.p99_latency_seconds, config.p99_latency_threshold
                );
                println!(
                    "  Active Requests: {} (threshold: {})",
                    system_metrics.active_requests, config.active_requests_threshold
                );
                println!(
                    "  Throughput: {:.0} req/s",
                    system_metrics.requests_per_second
                );

                // Check primary failure condition: error rate
                if system_metrics.error_rate_percent >= config.error_rate_threshold {
                    println!(
                        "\n🔴 LIMIT REACHED - Error Rate: {:.2}% ≥ {:.1}%",
                        system_metrics.error_rate_percent, config.error_rate_threshold
                    );
                    println!("\n╔══════════════════════════════════════════════════════════════════════════╗");
                    println!("║  Breaking Point Identified 🎯                                            ║");
                    println!("╚══════════════════════════════════════════════════════════════════════════╝");
                    println!();
                    println!("Maximum Capacity Found:");
                    println!("  Breaking Point: {} concurrent users", current_users);
                    println!("  Last Healthy Load: {} users", last_healthy_users);
                    println!(
                        "  Peak Throughput: {:.0} req/s (at {} users)",
                        last_healthy_throughput, last_healthy_users
                    );
                    println!(
                        "  Current Error Rate: {:.2}%",
                        system_metrics.error_rate_percent
                    );
                    println!();
                    println!("Recommendation:");
                    println!(
                        "  Set production limit to ~{} users (80% of last healthy)",
                        (last_healthy_users as f64 * 0.8) as usize
                    );
                    println!(
                        "  Expected throughput: ~{:.0} req/s",
                        last_healthy_throughput * 0.8
                    );
                    println!();

                    break;
                } else {
                    // System is healthy, record this state and continue
                    let mut warnings = Vec::new();

                    if system_metrics.p99_latency_seconds > config.p99_latency_threshold {
                        warnings.push(format!(
                            "⚠️  P99 latency {:.3}s > {:.2}s",
                            system_metrics.p99_latency_seconds, config.p99_latency_threshold
                        ));
                    }

                    if system_metrics.active_requests > config.active_requests_threshold {
                        warnings.push(format!(
                            "⚠️  Active requests {} > {}",
                            system_metrics.active_requests, config.active_requests_threshold
                        ));
                    }

                    if warnings.is_empty() {
                        println!("\n✅ System healthy - increasing load");
                    } else {
                        println!("\n⚠️  System functional but showing stress:");
                        for warning in &warnings {
                            println!("   {}", warning);
                        }
                        println!(
                            "   Error rate still acceptable ({:.2}%), continuing ramp-up",
                            system_metrics.error_rate_percent
                        );
                    }

                    // Update last known healthy state
                    last_healthy_users = current_users;
                    last_healthy_throughput = system_metrics.requests_per_second;
                }
            }
            Err(e) => {
                println!("\n⚠️  Warning: Could not query Prometheus: {}", e);
                println!("   Continuing test (assuming healthy)...");
                println!(
                    "   Tip: Ensure Prometheus is accessible at {}",
                    config.prometheus_url
                );

                last_healthy_users = current_users;
            }
        }

        // Check if we've hit the max users safety limit
        if current_users >= config.max_users {
            println!(
                "\n╔══════════════════════════════════════════════════════════════════════════╗"
            );
            println!(
                "║  Max Users Reached - No Failure Detected ✅                              ║"
            );
            println!(
                "╚══════════════════════════════════════════════════════════════════════════╝"
            );
            println!();
            println!("Safety Limit Hit:");
            println!("  Maximum Tested: {} users", current_users);
            println!("  Peak Throughput: {:.0} req/s", last_healthy_throughput);
            println!("  Error Rate: Still below threshold");
            println!();
            println!("Recommendation:");
            println!(
                "  System can handle ≥ {} users without errors",
                current_users
            );
            println!("  Consider increasing MAX_USERS to find actual limit");
            println!("  Or tighten ERROR_RATE_THRESHOLD for stricter SLA");
            break;
        }

        // Increment users for next cycle
        current_users += config.ramp_step;
        cycle += 1;
        println!();
    }

    Ok(())
}
