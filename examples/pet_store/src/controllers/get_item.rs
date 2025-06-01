
// User-owned controller for handler 'get_item'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::get_item::{ Request, Response };



#[handler(GetItemController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "id": "item-001",
        //   "name": "Sample Item"
        // }
    Response {
        id: Some("item-001".to_string()),
        name: Some("Sample Item".to_string()),
        
    }
    
}
