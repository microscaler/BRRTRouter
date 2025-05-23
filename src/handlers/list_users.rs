// Auto-generated by BRRTRouter

use crate::dispatcher::HandlerRequest;
use crate::typed::{TypedHandlerRequest, TypedHandlerResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Request {}

#[derive(Debug, Serialize)]
pub struct Response {}

pub fn handler(req: TypedHandlerRequest<Request>) -> Response {
    crate::controllers::list_users::handle(req)
}

impl From<HandlerRequest> for TypedHandlerRequest<Request> {
    fn from(_req: HandlerRequest) -> Self {
        // TODO: convert HandlerRequest to TypedHandlerRequest<Request>
        unimplemented!()
    }
}

impl From<TypedHandlerRequest<Request>> for HandlerRequest {
    fn from(_req: TypedHandlerRequest<Request>) -> Self {
        // TODO: convert TypedHandlerRequest<Request> to HandlerRequest
        unimplemented!()
    }
}
