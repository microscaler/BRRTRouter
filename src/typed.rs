// typed.rs
#[allow(unused_imports)]
use crate::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse};
use http::Method;
use may::sync::mpsc;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use anyhow::Result;
use std::collections::HashMap;

/// Trait implemented by typed coroutine handlers.
///
/// A handler receives a [`TypedHandlerRequest`] and returns a typed response.
pub trait Handler<TReq, TRes>: Send + 'static {
    fn handle(&self, req: TypedHandlerRequest<TReq>) -> TRes;
}

impl<TReq, TRes, F> Handler<TReq, TRes> for F
where
    F: Fn(TypedHandlerRequest<TReq>) -> TRes + Send + Sync + 'static,
{
    fn handle(&self, req: TypedHandlerRequest<TReq>) -> TRes {
        (self)(req)
    }
}

pub trait TypedHandlerFor<T>: Sized {
    fn from_handler(req: HandlerRequest) -> anyhow::Result<TypedHandlerRequest<T>>;
    fn into_handler(self) -> HandlerRequest;
}

#[derive(Debug, Clone)]
pub struct TypedHandlerRequest<T> {
    pub method: Method,
    pub path: String,
    pub handler_name: String,
    pub path_params: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub data: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypedHandlerResponse<T: Serialize> {
    pub status: u16,
    pub body: T,
}

impl<T> TypedHandlerFor<T> for TypedHandlerRequest<T>
where
    T: DeserializeOwned + Serialize,
{
    fn from_handler(req: HandlerRequest) -> Result<TypedHandlerRequest<T>> {
        use serde_json::{Map, Value};

        let mut data_map = Map::new();

        for (k, v) in &req.path_params {
            data_map.insert(k.clone(), Value::String(v.clone()));
        }
        for (k, v) in &req.query_params {
            data_map.insert(k.clone(), Value::String(v.clone()));
        }

        if let Some(body) = req.body.clone() {
            match body {
                Value::Object(map) => {
                    for (k, v) in map {
                        data_map.insert(k, v);
                    }
                }
                other => {
                    data_map.insert("body".to_string(), other);
                }
            }
        }

        let data: T = serde_json::from_value(Value::Object(data_map))?;

        Ok(TypedHandlerRequest {
            method: req.method,
            path: req.path,
            handler_name: req.handler_name,
            path_params: req.path_params,
            query_params: req.query_params,
            data,
        })
    }

    fn into_handler(self) -> HandlerRequest {
        let (tx, _rx) = mpsc::channel();
        HandlerRequest {
            method: self.method,
            path: self.path,
            handler_name: self.handler_name,
            path_params: self.path_params,
            query_params: self.query_params,
            body: serde_json::to_value(self.data).ok(),
            reply_tx: tx,
        }
    }
}

impl Dispatcher {
    /// Register a typed handler that deserializes the body into `TReq` and responds with `TRes`.
    pub unsafe fn register_typed<TReq, TRes, H>(&mut self, name: &str, handler: H)
    where
        TReq: DeserializeOwned + Send + 'static,
        TRes: Serialize + Send + 'static,
        H: Handler<TReq, TRes> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<HandlerRequest>();
        let name = name.to_string();

        may::coroutine::spawn(move || {
            let handler = handler;
            for req in rx.iter() {
                let reply_tx = req.reply_tx.clone();

                let typed_req = match TypedHandlerRequest::<TReq>::from_handler(req) {
                    Ok(v) => v,
                    Err(err) => {
                        let _ = reply_tx.send(HandlerResponse {
                            status: 400,
                            body: serde_json::json!({"error": "Invalid request data", "message": err.to_string()}),
                        });
                        continue;
                    }
                };

                let result = handler.handle(typed_req);

                let _ = reply_tx.send(HandlerResponse {
                    status: 200,
                    body: serde_json::to_value(result).unwrap_or_else(|_| {
                        serde_json::json!({
                            "error": "Failed to serialize response"
                        })
                    }),
                });
            }
        });

        self.handlers.insert(name, tx);
    }
}

// Example: typed handler
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePetRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePetResponse {
    pub id: String,
    pub name: String,
}

pub fn create_pet_handler(req: TypedHandlerRequest<CreatePetRequest>) -> CreatePetResponse {
    // Mock: assign static ID
    CreatePetResponse {
        id: "pet_1234".to_string(),
        name: req.data.name,
    }
}
