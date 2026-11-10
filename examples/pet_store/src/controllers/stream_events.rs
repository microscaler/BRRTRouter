// User-owned controller for handler 'stream_events'.

use crate::handlers::stream_events::Request;
use brrtrouter::sse;
use brrtrouter::typed::{HttpSse, TypedHandlerRequest};
use brrtrouter_macros::handler;
use std::time::Duration;

#[handler(StreamEventsController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> HttpSse {
    let (tx, rx) = sse::channel_bounded(sse::queue_bound_from_env());
    // Produce events on a coroutine so the service can flush early frames (Epic 13.7).
    may::go!(move || {
        for i in 0..3 {
            tx.send(format!("tick {i}"));
            may::coroutine::sleep(Duration::from_millis(20));
        }
    });
    HttpSse::new(rx)
}
