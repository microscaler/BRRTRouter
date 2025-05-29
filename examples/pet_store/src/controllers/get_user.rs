// User-owned controller for handler 'get_user'.
use crate::brrtrouter::typed::{Handler, TypedHandlerRequest};
use crate::handlers::get_user::{Request, Response};

pub struct GetUserController;

impl Handler for GetUserController {
    type Request = Request;
    type Response = Response;
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        // Example response:
        // {
        //   "id": "abc-123",
        //   "name": "John"
        // }
        Response {
            id: Some("abc-123".to_string()),
            name: Some("John".to_string()),
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    GetUserController.handle(req)
}
