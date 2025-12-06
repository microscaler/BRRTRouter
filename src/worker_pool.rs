//! # Worker Pool Module
//!
//! Provides worker pools for handler coroutines to enable parallel request processing.
//! Each handler can have multiple worker coroutines processing requests concurrently.
//!
//! ## Features
//!
//! - **Worker Pools**: Spawn N worker coroutines per handler for parallel request processing
//! - **Unbounded Queues**: Uses may's unbounded MPSC channels for maximum throughput
//! - **Load Balancing**: Workers automatically share the request queue
//! - **Metrics**: Track queue depth, dispatch count, and completion count for monitoring
//!
//! ## Configuration
//!
//! - `BRRTR_HANDLER_WORKERS`: Number of worker coroutines per handler (default: 4)
//! - `BRRTR_HANDLER_QUEUE_BOUND`: Queue depth limit for metrics (not enforced, default: 1024)
//! - `BRRTR_BACKPRESSURE_MODE`: Backpressure mode setting (not used, kept for compatibility)
//! - `BRRTR_BACKPRESSURE_TIMEOUT_MS`: Timeout setting (not used, kept for compatibility)

use crate::dispatcher::{HandlerRequest, HandlerResponse};
use may::sync::mpsc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info};

/// Configuration for worker pool backpressure behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureMode {
    /// Block the sender with a timeout, then retry
    Block,
    /// Shed the request immediately and return 429 (Too Many Requests)
    Shed,
}

impl BackpressureMode {
    /// Parse backpressure mode from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "block" => Some(Self::Block),
            "shed" => Some(Self::Shed),
            _ => None,
        }
    }
}

impl Default for BackpressureMode {
    fn default() -> Self {
        Self::Block
    }
}

/// Configuration for a worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Number of worker coroutines
    pub num_workers: usize,
    /// Maximum queue depth
    pub queue_bound: usize,
    /// Backpressure mode
    pub backpressure_mode: BackpressureMode,
    /// Timeout for block mode in milliseconds
    pub backpressure_timeout_ms: u64,
    /// Stack size for worker coroutines
    pub stack_size: usize,
}

impl WorkerPoolConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let num_workers = std::env::var("BRRTR_HANDLER_WORKERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);

        let queue_bound = std::env::var("BRRTR_HANDLER_QUEUE_BOUND")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1024);

        let backpressure_mode = std::env::var("BRRTR_BACKPRESSURE_MODE")
            .ok()
            .and_then(|s| BackpressureMode::from_str(&s))
            .unwrap_or_default();

        let backpressure_timeout_ms = std::env::var("BRRTR_BACKPRESSURE_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50);

        let stack_size = std::env::var("BRRTR_STACK_SIZE")
            .ok()
            .and_then(|s| {
                if let Some(hex) = s.strip_prefix("0x") {
                    usize::from_str_radix(hex, 16).ok()
                } else {
                    s.parse().ok()
                }
            })
            .unwrap_or(0x10000); // 64KB default

        Self {
            num_workers,
            queue_bound,
            backpressure_mode,
            backpressure_timeout_ms,
            stack_size,
        }
    }

    /// Create a custom configuration
    pub fn new(
        num_workers: usize,
        queue_bound: usize,
        backpressure_mode: BackpressureMode,
        backpressure_timeout_ms: u64,
        stack_size: usize,
    ) -> Self {
        Self {
            num_workers,
            queue_bound,
            backpressure_mode,
            backpressure_timeout_ms,
            stack_size,
        }
    }
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            num_workers: 4,
            queue_bound: 1024,
            backpressure_mode: BackpressureMode::Block,
            backpressure_timeout_ms: 50,
            stack_size: 0x10000, // 64KB
        }
    }
}

/// Metrics for a worker pool
#[derive(Debug)]
pub struct WorkerPoolMetrics {
    /// Number of requests shed due to queue overflow
    pub shed_count: AtomicU64,
    /// Current queue depth (approximate)
    pub queue_depth: AtomicUsize,
    /// Total requests dispatched
    pub dispatched_count: AtomicU64,
    /// Total requests completed
    pub completed_count: AtomicU64,
}

impl WorkerPoolMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self {
            shed_count: AtomicU64::new(0),
            queue_depth: AtomicUsize::new(0),
            dispatched_count: AtomicU64::new(0),
            completed_count: AtomicU64::new(0),
        }
    }

    /// Record a shed event
    pub fn record_shed(&self) {
        self.shed_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a dispatch event
    pub fn record_dispatch(&self) {
        self.dispatched_count.fetch_add(1, Ordering::Relaxed);
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a completion event
    pub fn record_completion(&self) {
        self.completed_count.fetch_add(1, Ordering::Relaxed);
        self.queue_depth.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current shed count
    pub fn get_shed_count(&self) -> u64 {
        self.shed_count.load(Ordering::Relaxed)
    }

    /// Get current queue depth
    pub fn get_queue_depth(&self) -> usize {
        self.queue_depth.load(Ordering::Relaxed)
    }

    /// Get total dispatched count
    pub fn get_dispatched_count(&self) -> u64 {
        self.dispatched_count.load(Ordering::Relaxed)
    }

    /// Get total completed count
    pub fn get_completed_count(&self) -> u64 {
        self.completed_count.load(Ordering::Relaxed)
    }
}

impl Default for WorkerPoolMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// A worker pool for handling requests with bounded queues and backpressure
pub struct WorkerPool {
    /// Configuration for the pool
    config: WorkerPoolConfig,
    /// Sender for dispatching requests to workers
    sender: mpsc::Sender<HandlerRequest>,
    /// Metrics for monitoring
    metrics: Arc<WorkerPoolMetrics>,
    /// Handler name for logging
    handler_name: String,
}

impl WorkerPool {
    /// Create a new worker pool with the given configuration and handler function
    ///
    /// # Safety
    ///
    /// This function is marked unsafe because it spawns coroutines using `may::coroutine::Builder::spawn()`,
    /// which is unsafe in the `may` runtime. The caller must ensure the May coroutine runtime is properly initialized.
    ///
    /// # Arguments
    ///
    /// * `handler_name` - Name of the handler for logging and metrics
    /// * `config` - Configuration for the worker pool
    /// * `handler_fn` - Function to handle requests (must be Send + 'static)
    pub unsafe fn new<F>(handler_name: String, config: WorkerPoolConfig, handler_fn: F) -> Self
    where
        F: Fn(HandlerRequest) + Send + 'static + Clone,
    {
        let (tx, rx) = mpsc::channel::<HandlerRequest>();
        let metrics = Arc::new(WorkerPoolMetrics::new());

        // Create a shared receiver wrapped in Arc for all workers to share
        let rx = Arc::new(rx);

        info!(
            handler_name = %handler_name,
            num_workers = config.num_workers,
            queue_bound = config.queue_bound,
            backpressure_mode = ?config.backpressure_mode,
            stack_size = config.stack_size,
            "Creating worker pool"
        );

        // Spawn worker coroutines
        for worker_id in 0..config.num_workers {
            let rx_clone = rx.clone();
            let handler_fn = handler_fn.clone();
            let handler_name_clone = handler_name.clone();
            let metrics_clone = metrics.clone();

            let spawn_result = may::coroutine::Builder::new()
                .stack_size(config.stack_size)
                .spawn(move || {
                    debug!(
                        handler_name = %handler_name_clone,
                        worker_id = worker_id,
                        "Worker coroutine started"
                    );

                    // Process requests until channel closes
                    // Note: All workers share the same receiver, so they will
                    // automatically load balance across incoming requests
                    loop {
                        match rx_clone.recv() {
                            Ok(req) => {
                                let request_id = req.request_id;
                                let handler_name = req.handler_name.clone();

                                debug!(
                                    request_id = %request_id,
                                    handler_name = %handler_name,
                                    worker_id = worker_id,
                                    "Worker processing request"
                                );

                                // Call the handler function with panic recovery
                                if let Err(panic) =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        handler_fn(req);
                                    }))
                                {
                                    // Handler panicked - send 500 error response
                                    error!(
                                        request_id = %request_id,
                                        handler_name = %handler_name,
                                        worker_id = worker_id,
                                        panic_message = ?panic,
                                        "Handler panicked - CRITICAL"
                                    );
                                }

                                // Record completion
                                metrics_clone.record_completion();
                            }
                            Err(_) => {
                                // Channel closed, exit worker
                                break;
                            }
                        }
                    }

                    debug!(
                        handler_name = %handler_name_clone,
                        worker_id = worker_id,
                        "Worker coroutine exiting"
                    );
                });

            if let Err(e) = spawn_result {
                error!(
                    handler_name = %handler_name,
                    worker_id = worker_id,
                    error = %e,
                    "Failed to spawn worker coroutine"
                );
            }
        }

        Self {
            config,
            sender: tx,
            metrics,
            handler_name,
        }
    }

    /// Dispatch a request to the worker pool
    ///
    /// Sends the request to the unbounded channel where worker coroutines will pick it up.
    /// Returns an error only if the channel is disconnected (workers have exited).
    ///
    /// # Arguments
    ///
    /// * `req` - The request to dispatch
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Request dispatched successfully
    /// * `Err(HandlerResponse)` - Channel disconnected (503 error response)
    pub fn dispatch(&self, req: HandlerRequest) -> Result<(), HandlerResponse> {
        // Both modes now behave the same - just send to the unbounded channel
        // We keep the mode check for future bounded channel implementation
        match self.config.backpressure_mode {
            BackpressureMode::Block => self.dispatch_with_blocking(req),
            BackpressureMode::Shed => self.dispatch_with_shedding(req),
        }
    }

    /// Dispatch to the unbounded channel (block mode - no actual blocking)
    fn dispatch_with_blocking(&self, req: HandlerRequest) -> Result<(), HandlerResponse> {
        let request_id = req.request_id;

        // Simply send to the unbounded channel
        // Note: The channel is unbounded, so this will always succeed unless disconnected
        self.metrics.record_dispatch();

        if let Err(e) = self.sender.send(req) {
            // Channel disconnected - workers are gone
            error!(
                request_id = %request_id,
                handler_name = %self.handler_name,
                error = %e,
                "Worker pool channel disconnected"
            );

            return Err(HandlerResponse::error(
                503,
                "Handler workers are not responding",
            ));
        }

        Ok(())
    }

    /// Dispatch to the unbounded channel (shed mode - no actual shedding)
    fn dispatch_with_shedding(&self, req: HandlerRequest) -> Result<(), HandlerResponse> {
        let request_id = req.request_id;

        // Simply send to the unbounded channel
        // Note: The channel is unbounded, so this will always succeed unless disconnected
        self.metrics.record_dispatch();

        if let Err(e) = self.sender.send(req) {
            // Channel disconnected - workers are gone
            error!(
                request_id = %request_id,
                handler_name = %self.handler_name,
                error = %e,
                "Worker pool channel disconnected"
            );

            return Err(HandlerResponse::error(
                503,
                "Handler workers are not responding",
            ));
        }

        Ok(())
    }

    /// Get the sender for this worker pool
    pub fn sender(&self) -> mpsc::Sender<HandlerRequest> {
        self.sender.clone()
    }

    /// Get metrics for this worker pool
    pub fn metrics(&self) -> &Arc<WorkerPoolMetrics> {
        &self.metrics
    }

    /// Get configuration for this worker pool
    pub fn config(&self) -> &WorkerPoolConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backpressure_mode_from_str() {
        assert_eq!(
            BackpressureMode::from_str("block"),
            Some(BackpressureMode::Block)
        );
        assert_eq!(
            BackpressureMode::from_str("Block"),
            Some(BackpressureMode::Block)
        );
        assert_eq!(
            BackpressureMode::from_str("BLOCK"),
            Some(BackpressureMode::Block)
        );
        assert_eq!(
            BackpressureMode::from_str("shed"),
            Some(BackpressureMode::Shed)
        );
        assert_eq!(
            BackpressureMode::from_str("Shed"),
            Some(BackpressureMode::Shed)
        );
        assert_eq!(
            BackpressureMode::from_str("SHED"),
            Some(BackpressureMode::Shed)
        );
        assert_eq!(BackpressureMode::from_str("invalid"), None);
    }

    #[test]
    fn test_worker_pool_config_default() {
        let config = WorkerPoolConfig::default();
        assert_eq!(config.num_workers, 4);
        assert_eq!(config.queue_bound, 1024);
        assert_eq!(config.backpressure_mode, BackpressureMode::Block);
        assert_eq!(config.backpressure_timeout_ms, 50);
        assert_eq!(config.stack_size, 0x10000);
    }

    #[test]
    fn test_worker_pool_metrics() {
        let metrics = WorkerPoolMetrics::new();

        assert_eq!(metrics.get_shed_count(), 0);
        assert_eq!(metrics.get_queue_depth(), 0);
        assert_eq!(metrics.get_dispatched_count(), 0);
        assert_eq!(metrics.get_completed_count(), 0);

        metrics.record_dispatch();
        assert_eq!(metrics.get_dispatched_count(), 1);
        assert_eq!(metrics.get_queue_depth(), 1);

        metrics.record_completion();
        assert_eq!(metrics.get_completed_count(), 1);
        assert_eq!(metrics.get_queue_depth(), 0);

        metrics.record_shed();
        assert_eq!(metrics.get_shed_count(), 1);
    }
}
