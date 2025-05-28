
// User-owned controller for handler 'list_user_posts'.
use crate::brrtrouter::typed::{TypedHandlerRequest, Handler};
use crate::handlers::list_user_posts::{ Request, Response };
use crate::handlers::types::Post;


pub struct ListUserPostsController;

impl Handler<Request, Response> for ListUserPostsController {
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        // Example response:
        // [
        //   {
        //     "body": "Welcome to the blog",
        //     "id": "post1",
        //     "title": "Intro"
        //   },
        //   {
        //     "body": "Thanks for reading",
        //     "id": "post2",
        //     "title": "Follow-up"
        //   }
        // ]
        Response {
            items: vec![Default::default()],
            
        }
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    ListUserPostsController.handle(req)
}
