use crate::dispatcher::{HandlerRequest, HandlerResponse};
use std::collections::HashMap;

// Example handler: just echoes back input for now
// This handler is useful for testing and debugging purposes.
#[allow(dead_code)]
pub fn echo_handler(req: HandlerRequest) {
    let response = HandlerResponse {
        status: 200,
        headers: HashMap::new(),
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

#[cfg(test)]
mod tests {
    use super::echo_handler;
    use crate::dispatcher::{HandlerRequest, HandlerResponse};
    use http::Method;
    use may::sync::mpsc;
    use serde_json::json;
    use std::collections::HashMap;
    use crate::ids::RequestId;

    #[test]
    fn test_echo_handler() {
        let (tx, rx) = mpsc::channel::<HandlerResponse>();
        let mut params = HashMap::new();
        params.insert("id".to_string(), "123".to_string());
        let mut query = HashMap::new();
        query.insert("debug".to_string(), "true".to_string());
        let body = json!({"name": "test"});

        let req = HandlerRequest {
            request_id: RequestId::new(),
            method: Method::POST,
            path: "/items/123".to_string(),
            handler_name: "echo".to_string(),
            path_params: params.clone(),
            query_params: query.clone(),
            headers: HashMap::new(),
            cookies: HashMap::new(),
            body: Some(body.clone()),
            reply_tx: tx,
        };

        echo_handler(req);
        let resp = rx.recv().unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(
            resp.body,
            json!({
                "handler": "echo",
                "method": "POST",
                "path": "/items/123",
                "params": params,
                "query": query,
                "body": Some(body)
            })
        );
    }
}
