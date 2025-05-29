use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct MetricsMiddleware {
    request_count: AtomicUsize,
    total_latency_ns: AtomicU64,
}

impl MetricsMiddleware {
    pub fn new() -> Self {
        Self {
            request_count: AtomicUsize::new(0),
            total_latency_ns: AtomicU64::new(0),
        }
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }

    pub fn average_latency(&self) -> Duration {
        let count = self.request_count.load(Ordering::Relaxed) as u64;
        if count == 0 {
            Duration::from_nanos(0)
        } else {
            Duration::from_nanos(self.total_latency_ns.load(Ordering::Relaxed) / count)
        }
    }
}

impl Middleware for MetricsMiddleware {
    fn before(&self, _req: &HandlerRequest) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    fn after(&self, _req: &HandlerRequest, _res: &HandlerResponse, latency: Duration) {
        self.total_latency_ns
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);
    }
}
