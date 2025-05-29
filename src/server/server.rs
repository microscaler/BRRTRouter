use crate::dispatcher::{Dispatcher, HandlerResponse};
use crate::router::Router;
use std::sync::{Arc, RwLock};
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
    pub router: Arc<RwLock<Router>>,
    pub dispatcher: Arc<RwLock<Dispatcher>>,
}

impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        let method_str = req.method().to_string();
        let method = method_str.as_str();
        let raw_path = req.path().to_string();
        let path = raw_path.split('?').next().unwrap_or("/").to_string();

        let headers: HashMap<String, String> = req
            .headers()
            .iter()
            .map(|h| {
                (
                    h.name.to_ascii_lowercase(),
                    String::from_utf8_lossy(h.value).to_string(),
                )
            })
            .collect();

        let cookies: HashMap<String, String> = headers
            .get("cookie")
            .map(|c| {
                c.split(';')
                    .filter_map(|pair| {
                        let mut parts = pair.trim().splitn(2, '=');
                        let name = parts.next()?.trim().to_string();
                        let value = parts.next().unwrap_or("").trim().to_string();
                        Some((name, value))
                    })
                    .collect()
            })
            .unwrap_or_default();

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

        let route_opt = {
            let router = self.router.read().unwrap();
            router.route(method.parse().unwrap(), &path)
        };
        if let Some(mut route_match) = route_opt {
            route_match.query_params = query_params.clone();
            let handler_response = {
                let dispatcher = self.dispatcher.read().unwrap();
                dispatcher.dispatch(route_match, body, headers, cookies)
            };

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
