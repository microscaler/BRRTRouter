// User-owned controller for handler 'stream_events'.

use crate::handlers::stream_events::{Request, Response};
use brrtrouter::typed::TypedHandlerRequest;
use brrtrouter_macros::handler;

use brrtrouter::sse;

#[handler(StreamEventsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    // Produce frames on a coroutine so the service flushes each one as it is sent (Epic 13.7).
    let (tx, rx) = sse::channel_bounded(sse::queue_bound_from_env());
    may::go!(move || {
        for i in 0..3 {
            tx.send(format!("tick {i}"));
            may::coroutine::sleep(std::time::Duration::from_millis(20));
        }
    });
    Response::new(rx)
}
