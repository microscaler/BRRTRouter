mod auth;
mod core;
mod cors;
mod metrics;
mod tracing;

pub use auth::AuthMiddleware;
pub use core::Middleware;
pub use cors::CorsMiddleware;
pub use metrics::MetricsMiddleware;
pub use tracing::TracingMiddleware;

// Simple console logging middleware for development
use std::time::Duration;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct ConsoleLoggingMiddleware;

impl Middleware for ConsoleLoggingMiddleware {
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        println!("→ {} {} ({})", req.method, req.path, req.handler_name);
        None
    }

    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        let status_color = if res.status >= 200 && res.status < 300 {
            "🟢"
        } else if res.status >= 400 {
            "🔴"
        } else {
            "🟡"
        };
        
        println!("← {} {} {} {}ms", 
                status_color, 
                res.status, 
                req.path, 
                latency.as_millis());
    }
}
