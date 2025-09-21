// User-owned controller for handler 'download_file'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::download_file::{Request, Response};
use brrtrouter_macros::handler;

#[handler(DownloadFileController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {}
}
