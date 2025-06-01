
// User-owned controller for handler 'post_item'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::post_item::{ Request, Response };



#[handler(PostItemController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
    // Example response:
        // {
        //   "id": "item-001",
        //   "name": "New Item"
        // }
    Response {
        id: Some("item-001".to_string()),
        name: Some("New Item".to_string()),
        
    }
    
}
