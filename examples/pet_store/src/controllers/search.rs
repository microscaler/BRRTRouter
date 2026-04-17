// User-owned controller for handler 'search'.

use crate::handlers::search::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[allow(unused_imports)]
use crate::handlers::types::Item;

#[handler(SearchController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Example response:
    // {
    //   "results": [
    //     {
    //       "id": "item-001",
    //       "name": "Sample Item"
    //     },
    //     {
    //       "id": "item-002",
    //       "name": "Another Item"
    //     }
    //   ]
    // }
    match serde_json::from_str::<Response>(
        r###"{
  "results": [
    {
      "id": "item-001",
      "name": "Sample Item"
    },
    {
      "id": "item-002",
      "name": "Another Item"
    }
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
        results: Some(vec![
            serde_json::from_value::<Item>(
                serde_json::json!({"id":"item-001","name":"Sample Item"}),
            )
            .unwrap_or_default(),
            serde_json::from_value::<Item>(
                serde_json::json!({"id":"item-002","name":"Another Item"}),
            )
            .unwrap_or_default(),
        ]),
    }
}
