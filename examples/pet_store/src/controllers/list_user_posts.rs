
// User-owned controller for handler 'list_user_posts'.
use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::list_user_posts::{ Request, Response };

pub struct ListUserPostsController;

impl Handler<Request, Response> for ListUserPostsController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        // Example response:
        // 
        Response {
            
            items: vec![Default::default()],
            
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    ListUserPostsController.handle(req)
}