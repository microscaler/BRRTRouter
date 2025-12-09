use anyhow::anyhow;
use brrtrouter::typed::TypedHandlerFor;
use brrtrouter::{
    dispatcher::{HandlerRequest, HandlerResponse, HeaderVec},
    router::ParamVec,
    typed::TypedHandlerRequest,
};
use http::Method;
use may::sync::mpsc;
use serde::{Deserialize, Serialize};
use smallvec::smallvec;
use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
struct Req {
    id: i32,
    active: bool,
}

impl TryFrom<HandlerRequest> for Req {
    type Error = anyhow::Error;

    fn try_from(req: HandlerRequest) -> Result<Self, Self::Error> {
        let id = req
            .get_path_param("id")
            .ok_or_else(|| anyhow::anyhow!("missing id"))?
            .parse()?;
        let active = req
            .get_query_param("active")
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
    let path_params: ParamVec = smallvec![(Arc::from("id"), "42".to_string())];
    let query_params: ParamVec = smallvec![(Arc::from("active"), "true".to_string())];

    let req = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/items/42".to_string(),
        handler_name: "test".to_string(),
        path_params: path_params,
        query_params: query_params,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
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
            .get_header("x-token")
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("missing token"))?;
        let session = req
            .get_cookie("session")
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("missing session"))?;
        Ok(HeaderCookieReq { token, session })
    }
}

#[test]
fn test_header_cookie_params() {
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    let headers: HeaderVec = smallvec![(Arc::from("x-token"), "secret".to_string())];
    let cookies: HeaderVec = smallvec![(Arc::from("session"), "abc123".to_string())];

    let req = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/items".to_string(),
        handler_name: "test".to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies,
        body: None,
        jwt_claims: None,
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
            .get_query_param("a")
            .ok_or_else(|| anyhow!("missing a"))?
            .parse()?;
        let b = req
            .get_query_param("b")
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
    let q: ParamVec = smallvec![
        (Arc::from("a"), "2".to_string()),
        (Arc::from("b"), "3".to_string())
    ];
    tx.send(HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/sum".into(),
        handler_name: "sum".into(),
        path_params: ParamVec::new(),
        query_params: q,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
    })
    .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body["total"], 5);

    let (reply_tx, reply_rx) = mpsc::channel();
    let bad_q: ParamVec = smallvec![(Arc::from("a"), "2".to_string())];
    tx.send(HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/sum".into(),
        handler_name: "sum".into(),
        path_params: ParamVec::new(),
        query_params: bad_q,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
    })
    .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 400);
}
