use crate::dispatcher::{Dispatcher, HandlerResponse};
use crate::router::Router;
use may_minihttp::{HttpService, Request, Response};
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::io::Read;

#[derive(Serialize, Default)]
#[allow(dead_code)]
struct JsonResponse {
    handler: String,
    method: String,
    path: String,
    params: HashMap<String, String>,
}

#[derive(Clone)]
pub struct AppService {
    pub router: Router,
    pub dispatcher: Dispatcher,
}

impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        let method_str = req.method().to_string();
        let method = method_str.as_str();
        let raw_path = req.path().to_string();
        let path = raw_path.split('?').next().unwrap_or("/").to_string();

        let query_params = {
            let path_str = req.path();
            if let Some(pos) = path_str.find('?') {
                let query_str = &path_str[pos + 1..];
                url::form_urlencoded::parse(query_str.as_bytes())
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect()
            } else {
                HashMap::new()
            }
        };

        let body = {
            let mut body_str = String::new();
            if let Ok(size) = req.body().read_to_string(&mut body_str) {
                if size > 0 {
                    serde_json::from_str(&body_str).ok()
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(mut route_match) = self.router.route(method.parse().unwrap(), &path) {
            route_match.query_params = query_params.clone();
            let handler_response = self.dispatcher.dispatch(route_match, body);

            match handler_response {
                Some(HandlerResponse { status, body }) => {
                    res.status_code(status as usize, "OK");
                    res.header("Content-Type: application/json");
                    res.body_vec(serde_json::to_vec(&body).unwrap());
                }
                None => {
                    res.status_code(500, "Internal Server Error");
                    res.header("Content-Type: application/json");
                    res.body_vec(
                        serde_json::json!({
                            "error": "Handler failed or not registered",
                            "method": method_str,
                            "path": path
                        })
                        .to_string()
                        .into_bytes(),
                    );
                }
            }
        } else {
            res.status_code(404, "Not Found");
            res.header("Content-Type: application/json");
            res.body_vec(
                serde_json::json!({
                    "error": "Not Found",
                    "method": method_str,
                    "path": path
                })
                .to_string()
                .into_bytes(),
            );
        }
        Ok(())
    }
}
