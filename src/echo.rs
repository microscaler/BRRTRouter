use crate::dispatcher::{HandlerRequest, HandlerResponse};
use serde_json::json;

// Example handler: just echoes back input for now
pub fn echo_handler(req: HandlerRequest) {
    let response = HandlerResponse {
        status: 200,
        body: serde_json::json!({
            "handler": req.handler_name,
            "method": req.method.to_string(),
            "path": req.path,
            "params": req.path_params,
            "query": req.query_params,
            "body": req.body,
        }),
    };

    let _ = req.reply_tx.send(response);
}
