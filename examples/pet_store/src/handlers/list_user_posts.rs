use serde::{ Deserialize, Serialize };
use crate::brrtrouter::typed::TypedHandlerRequest;

use crate::handlers::types::Post;
#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub user_id: String,
    }

#[derive(Debug, Serialize)]
pub struct Response {
    pub items: Vec<Post>,
    }

pub fn handler(req: TypedHandlerRequest<Request>) -> Response {
    crate::controllers::list_user_posts::handle(req)
}
