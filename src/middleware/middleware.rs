use std::time::Duration;

use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub trait Middleware: Send + Sync {
    fn before(&self, _req: &HandlerRequest) {}
    fn after(&self, _req: &HandlerRequest, _res: &HandlerResponse, _latency: Duration) {}
}
