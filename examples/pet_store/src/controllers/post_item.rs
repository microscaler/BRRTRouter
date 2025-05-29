// User-owned controller for handler 'post_item'.
use crate::brrtrouter::typed::{Handler, TypedHandlerRequest};
use crate::handlers::post_item::{Request, Response};

pub struct PostItemController;

impl Handler for PostItemController {
    type Request = Request;
    type Response = Response;
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
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
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    PostItemController.handle(req)
}
