#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec},
    ids::RequestId,
    router::ParamVec,
    worker_pool::{BackpressureMode, WorkerPoolConfig},
};
use http::Method;
use may::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

// These tests are affected by global env vars. Use a mutex to serialize access.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Helper to clean worker pool env vars to ensure test isolation
fn clean_worker_pool_env_vars() {
    std::env::remove_var("BRRTR_HANDLER_WORKERS");
    std::env::remove_var("BRRTR_HANDLER_QUEUE_BOUND");
    std::env::remove_var("BRRTR_BACKPRESSURE_MODE");
    std::env::remove_var("BRRTR_BACKPRESSURE_TIMEOUT_MS");
}

/// Test that worker pools are created with the correct configuration
#[test]
fn test_worker_pool_creation() {
    let _guard = ENV_MUTEX.lock().unwrap();
    clean_worker_pool_env_vars();

    // Initialize may runtime
    may::config().set_workers(2);

    let mut dispatcher = Dispatcher::new();

    // Register handler with worker pool (default config: 4 workers)
    unsafe {
        dispatcher.register_handler_with_pool("test_handler", move |req: HandlerRequest| {
            // Send response
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: serde_json::json!({"status": "ok"}),
            });
        });
    }

    // Verify the worker pool was created
    // Note: Worker pool handlers use the pool for dispatch, not the handlers map
    assert!(dispatcher.worker_pools.contains_key("test_handler"));
    // Handler entry is NOT created - dispatch goes through the worker pool
    assert!(!dispatcher.handlers.contains_key("test_handler"));

    // Get the pool and verify config
    let pool = dispatcher.worker_pools.get("test_handler").unwrap();
    let config = pool.config();

    // Default config should have 4 workers and 1024 queue bound
    assert_eq!(config.num_workers, 4);
    assert_eq!(config.queue_bound, 1024);
    assert_eq!(config.backpressure_mode, BackpressureMode::Block);
}

/// Test that worker pool accepts all requests (unbounded queue)
#[test]
fn test_worker_pool_shed_mode() {
    // Initialize may runtime
    may::config().set_workers(2);

    let mut dispatcher = Dispatcher::new();

    // Create a config with shed mode - note that queue bounds are not enforced
    let config = WorkerPoolConfig::new(
        1, // 1 worker
        2, // queue bound (not enforced - for metrics only)
        BackpressureMode::Shed,
        50,      // timeout (not used)
        0x10000, // stack size
    );

    // Register handler that takes some time to process
    unsafe {
        dispatcher.register_handler_with_pool_config(
            "slow_handler",
            move |req: HandlerRequest| {
                // Simulate slow processing
                may::coroutine::sleep(Duration::from_millis(100));

                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: serde_json::json!({"status": "ok"}),
                });
            },
            config,
        );
    }

    // Get the worker pool to test dispatch directly
    let pool = dispatcher
        .worker_pools
        .get("slow_handler")
        .expect("Pool not found")
        .clone();

    // Send requests - all should be accepted since queue is unbounded
    let mut success_count = 0;
    for _i in 0..10 {
        let (reply_tx, _reply_rx) = mpsc::channel();
        let req = HandlerRequest {
            request_id: RequestId::new(),
            method: Method::GET,
            path: "/test".to_string(),
            handler_name: "slow_handler".to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx,
        };

        match pool.dispatch(req) {
            Ok(()) => {
                success_count += 1;
            }
            Err(response) => {
                // Channel disconnected - should not happen in this test
                panic!("Unexpected error response: status={}", response.status);
            }
        }
    }

    // All requests should be accepted since queue is unbounded
    assert_eq!(success_count, 10, "Expected all 10 requests to be accepted");
}

/// Test that backpressure in block mode waits and retries
#[test]
fn test_worker_pool_block_mode() {
    // Initialize may runtime
    may::config().set_workers(2);

    let mut dispatcher = Dispatcher::new();

    // Create a config with block mode and small queue
    let config = WorkerPoolConfig::new(
        1, // 1 worker
        2, // queue bound of 2
        BackpressureMode::Block,
        100,     // timeout 100ms
        0x10000, // stack size
    );

    // Register handler that processes quickly
    unsafe {
        dispatcher.register_handler_with_pool_config(
            "fast_handler",
            move |req: HandlerRequest| {
                // Simulate fast processing
                may::coroutine::sleep(Duration::from_millis(5));

                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: serde_json::json!({"status": "ok"}),
                });
            },
            config,
        );
    }

    // Get the worker pool to test dispatch directly
    let pool = dispatcher
        .worker_pools
        .get("fast_handler")
        .expect("Pool not found")
        .clone();

    // Send a moderate number of requests
    // With block mode, they should eventually all be accepted (unless timeout)
    let mut success_count = 0;
    let mut _timeout_count = 0;

    for _i in 0..20 {
        let (reply_tx, _reply_rx) = mpsc::channel();
        let req = HandlerRequest {
            request_id: RequestId::new(),
            method: Method::GET,
            path: "/test".to_string(),
            handler_name: "fast_handler".to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx,
        };

        match pool.dispatch(req) {
            Ok(()) => {
                success_count += 1;
                // Don't wait for response - we're just testing dispatch
            }
            Err(response) => {
                // Request timed out waiting
                assert_eq!(
                    response.status, 429,
                    "Expected 429 status for timed out request"
                );
                _timeout_count += 1;
            }
        }

        // Small delay between requests
        may::coroutine::sleep(Duration::from_millis(1));
    }

    // Most requests should succeed in block mode (some might timeout under high load)
    assert!(
        success_count > 15,
        "Expected most requests to succeed in block mode, but only {success_count} succeeded"
    );
}

/// Test worker pool metrics
#[test]
fn test_worker_pool_metrics() {
    // Initialize may runtime
    may::config().set_workers(2);

    let mut dispatcher = Dispatcher::new();

    // Register handler
    unsafe {
        dispatcher.register_handler_with_pool("metrics_handler", move |req: HandlerRequest| {
            may::coroutine::sleep(Duration::from_millis(10));

            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: serde_json::json!({"status": "ok"}),
            });
        });
    }

    // Get initial metrics
    let metrics_before = dispatcher.worker_pool_metrics();
    assert!(metrics_before.contains_key("metrics_handler"));

    let pool = dispatcher
        .worker_pools
        .get("metrics_handler")
        .expect("Pool not found")
        .clone();

    // Send some requests
    for _i in 0..5 {
        let (reply_tx, _reply_rx) = mpsc::channel();
        let req = HandlerRequest {
            request_id: RequestId::new(),
            method: Method::GET,
            path: "/test".to_string(),
            handler_name: "metrics_handler".to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx,
        };

        let _ = pool.dispatch(req);
    }

    // Wait a bit for processing
    may::coroutine::sleep(Duration::from_millis(100));

    // Get metrics after
    let metrics_after = dispatcher.worker_pool_metrics();
    let (queue_depth, _shed_count, dispatched, _completed) =
        metrics_after.get("metrics_handler").unwrap();

    // Check that we dispatched some requests
    assert!(
        *dispatched >= 5,
        "Expected at least 5 dispatched, got {dispatched}"
    );

    // Queue should be draining or empty
    assert!(
        *queue_depth <= 5,
        "Queue depth should be reasonable, got {queue_depth}"
    );
}

/// Test that configuration from environment variables works
#[test]
fn test_worker_pool_config_from_env() {
    let _guard = ENV_MUTEX.lock().unwrap();
    clean_worker_pool_env_vars();

    // Set environment variables
    std::env::set_var("BRRTR_HANDLER_WORKERS", "8");
    std::env::set_var("BRRTR_HANDLER_QUEUE_BOUND", "2048");
    std::env::set_var("BRRTR_BACKPRESSURE_MODE", "shed");
    std::env::set_var("BRRTR_BACKPRESSURE_TIMEOUT_MS", "100");

    let config = WorkerPoolConfig::from_env();

    assert_eq!(config.num_workers, 8);
    assert_eq!(config.queue_bound, 2048);
    assert_eq!(config.backpressure_mode, BackpressureMode::Shed);
    assert_eq!(config.backpressure_timeout_ms, 100);

    clean_worker_pool_env_vars();
}
