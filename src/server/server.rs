use crate::dispatcher::{Dispatcher, HandlerResponse};
use crate::router::Router;
use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::{SecurityScheme};
use may_minihttp::{HttpService, Request, Response};
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::sync::{Arc, RwLock};

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
    pub security_schemes: HashMap<String, SecurityScheme>,
    pub security_providers: HashMap<String, Arc<dyn SecurityProvider>>,
}

impl AppService {
    pub fn new(
        router: Arc<RwLock<Router>>,
        dispatcher: Arc<RwLock<Dispatcher>>,
        security_schemes: HashMap<String, SecurityScheme>,
    ) -> Self {
        Self {
            router,
            dispatcher,
            security_schemes,
            security_providers: HashMap::new(),
        }
    }

    pub fn register_security_provider(
        &mut self,
        name: &str,
        provider: Arc<dyn SecurityProvider>,
    ) {
        self.security_providers.insert(name.to_string(), provider);
    }
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
            if !route_match.route.security.is_empty() {
                let sec_req = SecurityRequest {
                    headers: &headers,
                    query: &query_params,
                    cookies: &cookies,
                };
                let mut authorized = false;
                'outer: for requirement in &route_match.route.security {
                    let mut ok = true;
                    for (scheme_name, scopes) in &requirement.0 {
                        let scheme = match self.security_schemes.get(scheme_name) {
                            Some(s) => s,
                            None => { ok = false; break; }
                        };
                        let provider = match self.security_providers.get(scheme_name) {
                            Some(p) => p,
                            None => { ok = false; break; }
                        };
                        if !provider.validate(scheme, scopes, &sec_req) {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        authorized = true;
                        break 'outer;
                    }
                }
                if !authorized {
                    res.status_code(401, "Unauthorized");
                    res.header("Content-Type: application/json");
                    res.body_vec(
                        serde_json::json!({"error": "Unauthorized"}).to_string().into_bytes(),
                    );
                    return Ok(());
                }
            }
            let is_sse = route_match.route.sse;
            let handler_response = {
                let dispatcher = self.dispatcher.read().unwrap();
                dispatcher.dispatch(route_match, body, headers, cookies)
            };
            match handler_response {
                Some(HandlerResponse { status, body }) => {
                    let reason = match status {
                        200 => "OK",
                        201 => "Created",
                        400 => "Bad Request",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        _ => "OK",
                    };
                    res.status_code(status as usize, reason);
                    match body {
                        serde_json::Value::String(s) => {
                            if is_sse {
                                res.header("Content-Type: text/event-stream");
                            } else {
                                res.header("Content-Type: text/plain");
                            }
                            res.body_vec(s.into_bytes());
                        }
                        other => {
                            res.header("Content-Type: application/json");
                            res.body_vec(serde_json::to_vec(&other).unwrap());
                        }
                    }
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
