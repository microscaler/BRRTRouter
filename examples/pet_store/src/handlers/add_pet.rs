use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(
    request(name: String),
    response(id: Option<i32>, status: Option<String>)
)]
pub fn handler(req: TypedHandlerRequest<Request>) -> Response {
    crate::controllers::add_pet::handle(req)
}
