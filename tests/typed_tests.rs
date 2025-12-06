use anyhow::anyhow;
use brrtrouter::typed::TypedHandlerFor;
use brrtrouter::{
    dispatcher::{HandlerRequest, HandlerResponse},
    typed::TypedHandlerRequest,
};
use http::Method;
use may::sync::mpsc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;

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
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/items/42".to_string(),
        handler_name: "test".to_string(),
        path_params: path_params.clone(),
        query_params: query_params.clone(),
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
        reply_tx: tx,
    };

    let typed = TypedHandlerRequest::<Req>::from_handler(req).expect("conversion failed");
    assert_eq!(typed.data.id, 42);
    assert!(typed.data.active);
}

#[derive(Debug, Deserialize, Serialize)]
struct HeaderCookieReq {
    token: String,
    session: String,
}

impl TryFrom<HandlerRequest> for HeaderCookieReq {
    type Error = anyhow::Error;

    fn try_from(req: HandlerRequest) -> Result<Self, Self::Error> {
        let token = req
            .headers
            .get("x-token")
            .cloned()
            .ok_or_else(|| anyhow!("missing token"))?;
        let session = req
            .cookies
            .get("session")
            .cloned()
            .ok_or_else(|| anyhow!("missing session"))?;
        Ok(HeaderCookieReq { token, session })
    }
}

#[test]
fn test_header_cookie_params() {
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let mut headers = HashMap::new();
    headers.insert("x-token".to_string(), "secret".to_string());
    let mut cookies = HashMap::new();
    cookies.insert("session".to_string(), "abc123".to_string());

    let req = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/items".to_string(),
        handler_name: "test".to_string(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers,
        cookies,
        body: None,
        reply_tx: tx,
    };

    let typed = TypedHandlerRequest::<HeaderCookieReq>::from_handler(req).unwrap();
    assert_eq!(typed.data.token, "secret");
    assert_eq!(typed.data.session, "abc123");
}

#[derive(Clone)]
struct SumHandler;

#[derive(Debug, Deserialize)]
struct SumReq {
    a: i32,
    b: i32,
}

impl TryFrom<HandlerRequest> for SumReq {
    type Error = anyhow::Error;
    fn try_from(req: HandlerRequest) -> Result<Self, Self::Error> {
        let a = req
            .query_params
            .get("a")
            .ok_or_else(|| anyhow!("missing a"))?
            .parse()?;
        let b = req
            .query_params
            .get("b")
            .ok_or_else(|| anyhow!("missing b"))?
            .parse()?;
        Ok(SumReq { a, b })
    }
}

#[derive(Serialize)]
struct SumResp {
    total: i32,
}

impl brrtrouter::typed::Handler for SumHandler {
    type Request = SumReq;
    type Response = SumResp;

    fn handle(&self, req: brrtrouter::typed::TypedHandlerRequest<Self::Request>) -> Self::Response {
        SumResp {
            total: req.data.a + req.data.b,
        }
    }
}

#[test]
fn test_spawn_typed_success_and_error() {
    let tx = unsafe { brrtrouter::typed::spawn_typed(SumHandler) };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut q = HashMap::new();
    q.insert("a".to_string(), "2".to_string());
    q.insert("b".to_string(), "3".to_string());
    tx.send(HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/sum".into(),
        handler_name: "sum".into(),
        path_params: HashMap::new(),
        query_params: q.clone(),
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
        reply_tx,
    })
    .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body["total"], 5);

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut bad_q = HashMap::new();
    bad_q.insert("a".to_string(), "2".to_string());
    tx.send(HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/sum".into(),
        handler_name: "sum".into(),
        path_params: HashMap::new(),
        query_params: bad_q,
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
        reply_tx,
    })
    .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 400);
}
