// typed.rs
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
#[allow(unused_imports)]
use crate::dispatcher::{HandlerRequest, HandlerResponse, Dispatcher};
use may::sync::mpsc;
use http::Method;

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

impl Dispatcher {
    /// Register a typed handler that deserializes the body into `TReq` and responds with `TRes`
    pub unsafe fn register_typed<TReq, TRes, F>(&mut self, name: &str, handler_fn: F)
    where
        TReq: DeserializeOwned + Send + 'static,
        TRes: Serialize + Send + 'static,
        F: Fn(TypedHandlerRequest<TReq>) -> TRes + Send + 'static + Clone,
    {
        let (tx, rx) = mpsc::channel::<HandlerRequest>();
        let name = name.to_string();

        may::coroutine::spawn(move || {
            for req in rx.iter() {
                let data: TReq = match req.body {
                    Some(json) => match serde_json::from_value(json) {
                        Ok(v) => v,
                        Err(err) => {
                            let _ = req.reply_tx.send(HandlerResponse {
                                status: 400,
                                body: serde_json::json!({
                                    "error": "Invalid request body",
                                    "message": err.to_string()
                                }),
                            });
                            continue;
                        }
                    },
                    None => {
                        let _ = req.reply_tx.send(HandlerResponse {
                            status: 400,
                            body: serde_json::json!({
                                "error": "Missing request body"
                            }),
                        });
                        continue;
                    }
                };

                let typed_req = TypedHandlerRequest {
                    method: req.method,
                    path: req.path,
                    handler_name: req.handler_name,
                    path_params: req.path_params,
                    query_params: req.query_params,
                    data,
                };

                let result = handler_fn(typed_req);

                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    body: serde_json::to_value(result).unwrap_or_else(|_| serde_json::json!({
                        "error": "Failed to serialize response"
                    })),
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
