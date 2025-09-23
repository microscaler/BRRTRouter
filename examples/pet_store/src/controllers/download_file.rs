// User-owned controller for handler 'download_file'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::download_file::{Request, Response};
use brrtrouter_macros::handler;

#[handler(DownloadFileController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "id": "abc",
    //   "url": "https://cdn.example.com/abc"
    // }

    Response {
        id: Some("abc".to_string()),
        url: Some("https://cdn.example.com/abc".to_string()),
    }
}
