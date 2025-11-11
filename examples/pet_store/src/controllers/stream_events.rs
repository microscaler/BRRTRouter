// User-owned controller for handler 'stream_events'.
use crate::handlers::stream_events::{Request, Response};
use brrtrouter::sse;
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

#[handler(StreamEventsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    let (tx, rx) = sse::channel();
    for i in 0..3 {
        tx.send(format!("tick {i}"));
    }
    drop(tx);
    Response(rx.collect())
}
