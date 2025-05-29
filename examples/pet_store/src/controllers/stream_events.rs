use crate::brrtrouter::sse;
use crate::brrtrouter::dispatcher::{HandlerRequest, HandlerResponse};
use std::time::Duration;

pub fn handle(req: HandlerRequest) {
    let (tx, rx) = sse::channel();
    // spawn a coroutine to emit periodic events
    may::go!(move || {
        for i in 0..3 {
            tx.send(format!("tick {i}"));
            may::coroutine::sleep(Duration::from_millis(50));
        }
    });
    let body = rx.collect();
    let resp = HandlerResponse { status: 200, body: serde_json::Value::String(body) };
    let _ = req.reply_tx.send(resp);
}
