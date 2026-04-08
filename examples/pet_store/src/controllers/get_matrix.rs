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
    match serde_json::from_str::<Response>(
        r###"{
  "coords": [
    1,
    2,
    3
  ]
}"###,
    ) {
        Ok(parsed) => return parsed,
        Err(e) => {
            eprintln!("Failed to parse mock example JSON into Response: {}", e);
            // Fallback to empty default structs below
        }
    }

    Response {
        coords: Some(vec![1, 2, 3]),
    }
}
