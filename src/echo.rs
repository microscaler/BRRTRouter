use crate::dispatcher::{HandlerRequest, HandlerResponse};

// Example handler: just echoes back input for now
// This handler is useful for testing and debugging purposes.
#[allow(dead_code)]
pub fn echo_handler(req: HandlerRequest) {
    // Convert Arc<str> params to String for JSON serialization
    let params: Vec<(String, String)> = req
        .path_params
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let query: Vec<(String, String)> = req
        .query_params
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();

    let response = HandlerResponse::json(
        200,
        serde_json::json!({
            "handler": req.handler_name,
            "method": req.method.to_string(),
            "path": req.path,
            "params": params,
            "query": query,
            "body": req.body,
        }),
    );

    let _ = req.reply_tx.send(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
    use crate::ids::RequestId;
    use crate::router::ParamVec;
    use http::Method;
    use may::sync::mpsc;
    use serde_json::json;
    use smallvec::smallvec;
    use std::sync::Arc;

    #[test]
    fn test_echo_handler() {
        let (tx, rx) = mpsc::channel::<HandlerResponse>();
        // JSF: Use Arc<str> for param names
        let params: ParamVec = smallvec![(Arc::from("id"), "123".to_string())];
        let query: ParamVec = smallvec![(Arc::from("debug"), "true".to_string())];
        let body = json!({"name": "test"});

        let req = HandlerRequest {
            request_id: RequestId::new(),
            method: Method::POST,
            path: "/items/123".to_string(),
            handler_name: "echo".to_string(),
            path_params: params.clone(),
            query_params: query.clone(),
            headers: HeaderVec::new(),
            cookies: HeaderVec::new(),
            body: Some(body.clone()),
            reply_tx: tx,
        };

        echo_handler(req);
        let resp = rx.recv().unwrap();
        assert_eq!(resp.status, 200);
        // Expected params/query as Vec for JSON comparison
        let expected_params = vec![("id".to_string(), "123".to_string())];
        let expected_query = vec![("debug".to_string(), "true".to_string())];
        assert_eq!(
            resp.body,
            json!({
                "handler": "echo",
                "method": "POST",
                "path": "/items/123",
                "params": expected_params,
                "query": expected_query,
                "body": Some(body)
            })
        );
    }
}
