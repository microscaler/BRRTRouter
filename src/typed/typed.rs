// typed.rs
#[allow(unused_imports)]
use crate::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse};
use anyhow::Result;
use http::Method;
use may::sync::mpsc;
use serde::Serialize;
use serde_json;
use std::collections::HashMap;
use std::convert::TryFrom;

/// Trait implemented by typed coroutine handlers.
///
/// A handler receives a [`TypedHandlerRequest`] and returns a typed response.
pub trait Handler: Send + 'static {
    type Request: TryFrom<HandlerRequest, Error = anyhow::Error> + Send + 'static;
    type Response: Serialize + Send + 'static;

    fn handle(&self, req: TypedHandlerRequest<Self::Request>) -> Self::Response;
}

pub trait TypedHandlerFor<T>: Sized {
    fn from_handler(req: HandlerRequest) -> anyhow::Result<TypedHandlerRequest<T>>;
}

/// Spawn a typed handler coroutine and return a sender to communicate with it.
pub unsafe fn spawn_typed<H>(handler: H) -> mpsc::Sender<HandlerRequest>
where
    H: Handler + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<HandlerRequest>();

    may::coroutine::Builder::new()
        .stack_size(may::config().get_stack_size())
        .spawn(move || {
            let handler = handler;
            for req in rx.iter() {
                let reply_tx = req.reply_tx.clone();
                let handler_name = req.handler_name.clone();

                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let reply_tx_inner = reply_tx.clone();

                    let data = match H::Request::try_from(req.clone()) {
                        Ok(v) => v,
                        Err(err) => {
                            let _ = reply_tx_inner.send(HandlerResponse {
                                status: 400,
                                headers: HashMap::new(),
                                body: serde_json::json!({
                                    "error": "Invalid request data",
                                    "message": err.to_string()
                                }),
                            });
                            return;
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

                    let result = handler.handle(typed_req);

                    let _ = reply_tx_inner.send(HandlerResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: serde_json::to_value(result).unwrap_or_else(
                            |_| serde_json::json!({"error": "Failed to serialize response"}),
                        ),
                    });
                }));

                if let Err(panic) = result {
                    let _ = reply_tx.send(HandlerResponse {
                        status: 500,
                        headers: HashMap::new(),
                        body: serde_json::json!({
                            "error": "Handler panicked",
                            "details": format!("{:?}", panic)
                        }),
                    });
                    eprintln!("Handler '{}' panicked: {:?}", handler_name, panic);
                }
            }
        })
        .unwrap();

    tx
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

impl<T> TypedHandlerFor<T> for TypedHandlerRequest<T>
where
    T: TryFrom<HandlerRequest, Error = anyhow::Error>,
{
    fn from_handler(req: HandlerRequest) -> Result<TypedHandlerRequest<T>> {
        let data = T::try_from(req.clone())?;

        Ok(TypedHandlerRequest {
            method: req.method,
            path: req.path,
            handler_name: req.handler_name,
            path_params: req.path_params,
            query_params: req.query_params,
            data,
        })
    }
}

impl Dispatcher {
    /// Register a typed handler that converts [`HandlerRequest`] into the handler's
    /// associated request type using `TryFrom`.
    pub unsafe fn register_typed<H>(&mut self, name: &str, handler: H)
    where
        H: Handler + Send + 'static,
    {
        let name = name.to_string();
        let tx = spawn_typed(handler);
        self.handlers.insert(name, tx);
    }
}
