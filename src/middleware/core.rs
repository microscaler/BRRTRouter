use std::time::Duration;

use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub trait Middleware: Send + Sync {
    fn before(&self, _req: &HandlerRequest) -> Option<HandlerResponse> {
        None
    }
    fn after(&self, _req: &HandlerRequest, _res: &mut HandlerResponse, _latency: Duration) {}
}
