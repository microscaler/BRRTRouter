// User-owned controller for handler 'get_matrix'.
use crate::handlers::get_matrix::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(GetMatrixController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "coords": [
    //     1,
    //     2,
    //     3
    //   ]
    // }

    Response {
        coords: Some(vec![1, 2, 3]),
    }
}
