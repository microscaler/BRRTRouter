// User-owned controller for handler 'upload_file'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::upload_file::{Request, Response};
use brrtrouter_macros::handler;

#[handler(UploadFileController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
