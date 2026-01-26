
// User-owned controller for handler 'search'.
use brrtrouter_macros::handler;
use brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::search::{ Request, Response };


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
    
    Response {
        results: Some(vec![serde_json::from_value::<Item>(serde_json::json!({"id":"item-001","name":"Sample Item"})).unwrap_or_default(), serde_json::from_value::<Item>(serde_json::json!({"id":"item-002","name":"Another Item"})).unwrap_or_default()]),
    }
    
    
}