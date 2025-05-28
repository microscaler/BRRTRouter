// User-owned controller for handler 'get_item'.
use crate::brrtrouter::typed::{Handler, TypedHandlerRequest};
use crate::handlers::get_item::{Request, Response};

pub struct GetItemController;

impl Handler<Request, Response> for GetItemController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
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
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    GetItemController.handle(req)
}
