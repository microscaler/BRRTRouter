// User-owned controller for handler 'search'.
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::search::{Request, Response};
use brrtrouter_macros::handler;

#[allow(unused_imports)]
use crate::handlers::types::Item;

#[handler(SearchController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        results: Some(Default::default()),
    }
}
