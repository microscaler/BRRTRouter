// User-owned controller for handler 'register_webhook'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::register_webhook::{Request, Response};
use brrtrouter_macros::handler;

#[handler(RegisterWebhookController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
