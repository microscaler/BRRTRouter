// User-owned controller for handler 'stream_events'.
use crate::brrtrouter::sse;
use crate::brrtrouter::typed::{Handler, TypedHandlerRequest};
use crate::handlers::stream_events::{Request, Response};

pub struct StreamEventsController;

impl Handler for StreamEventsController {
    type Request = Request;
    type Response = Response;
    fn handle(&self, _req: TypedHandlerRequest<Request>) -> Response {
        let (tx, rx) = sse::channel();
        for i in 0..3 {
            tx.send(format!("tick {}", i));
        }
        drop(tx);
        Response(rx.collect())
    }
}

pub fn handle(req: TypedHandlerRequest<Request>) -> Response {
    StreamEventsController.handle(req)
}
