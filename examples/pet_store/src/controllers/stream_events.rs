
use crate::brrtrouter::typed::{Handler, TypedHandlerRequest};
use crate::handlers::stream_events::{Request, Response};

pub struct StreamEventsController;

impl Handler for StreamEventsController {
    type Request = Request;
    type Response = Response;
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        Response {}
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    StreamEventsController.handle(req)

}
