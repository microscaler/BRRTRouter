
// User-owned controller for handler 'list_user_posts'.
use brrtrouter_macros::handler;
use crate::brrtrouter::typed::TypedHandlerRequest;
use crate::handlers::list_user_posts::{ Request, Response };

use crate::handlers::types::Post;


#[handler(ListUserPostsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    
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
    
    Response(vec![Default::default()])
    
    
}
