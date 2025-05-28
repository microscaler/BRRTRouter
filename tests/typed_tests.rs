use brrtrouter::typed::TypedHandlerFor;
use brrtrouter::{
    dispatcher::{HandlerRequest, HandlerResponse},
    typed::TypedHandlerRequest,
};
use http::Method;
use may::sync::mpsc;
use std::convert::TryFrom;
use serde::{Deserialize, Serialize};
use anyhow::anyhow;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
struct Req {
    id: i32,
    active: bool,
}

impl TryFrom<HandlerRequest> for Req {
    type Error = anyhow::Error;

    fn try_from(req: HandlerRequest) -> Result<Self, Self::Error> {
        let id = req
            .path_params
            .get("id")
            .ok_or_else(|| anyhow::anyhow!("missing id"))?
            .parse()?;
        let active = req
            .query_params
            .get("active")
            .map(|v| v.parse::<bool>())
            .transpose()?;
        Ok(Req {
            id,
            active: active.unwrap_or(false),
        })
    }
}

#[test]
fn test_from_handler_non_string_params() {
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let mut path_params = HashMap::new();
    path_params.insert("id".to_string(), "42".to_string());
    let mut query_params = HashMap::new();
    query_params.insert("active".to_string(), "true".to_string());

    let req = HandlerRequest {
        method: Method::GET,
        path: "/items/42".to_string(),
        handler_name: "test".to_string(),
        path_params: path_params.clone(),
        query_params: query_params.clone(),
        body: None,
        reply_tx: tx,
    };

    let typed = TypedHandlerRequest::<Req>::from_handler(req).expect("conversion failed");
    assert_eq!(typed.data.id, 42);
    assert!(typed.data.active);
}
