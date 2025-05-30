use super::request::{parse_request, ParsedRequest};
use super::response::{write_handler_response, write_json_error};
use crate::dispatcher::{Dispatcher, HandlerResponse};
use crate::router::Router;
use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use crate::middleware::MetricsMiddleware;
use may_minihttp::{HttpService, Request, Response};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppService {
    pub router: Arc<RwLock<Router>>,
    pub dispatcher: Arc<RwLock<Dispatcher>>,
    pub security_schemes: HashMap<String, SecurityScheme>,
    pub security_providers: HashMap<String, Arc<dyn SecurityProvider>>,
    pub metrics: Option<Arc<crate::middleware::MetricsMiddleware>>,
    pub spec_path: PathBuf,
}

impl AppService {
    pub fn new(
        router: Arc<RwLock<Router>>,
        dispatcher: Arc<RwLock<Dispatcher>>,
        security_schemes: HashMap<String, SecurityScheme>,
        spec_path: PathBuf,
    ) -> Self {
        Self {
            router,
            dispatcher,
            security_schemes,
            security_providers: HashMap::new(),
            metrics: None,
            spec_path,
        }
    }

    pub fn register_security_provider(&mut self, name: &str, provider: Arc<dyn SecurityProvider>) {
        self.security_providers.insert(name.to_string(), provider);
    }

    pub fn set_metrics_middleware(&mut self, metrics: Arc<MetricsMiddleware>) {
        self.metrics = Some(metrics);
    }
}

/// Basic health check endpoint returning `{ "status": "ok" }`.
pub fn health_endpoint(res: &mut Response) -> io::Result<()> {
    write_handler_response(
        res,
        200,
        serde_json::json!({ "status": "ok" }),
        false,
        &HashMap::new(),
    );
    Ok(())
}

/// Metrics endpoint returning Prometheus text format statistics.
pub fn metrics_endpoint(res: &mut Response, metrics: &MetricsMiddleware) -> io::Result<()> {
    let body = format!(
        "# HELP brrtrouter_requests_total Total number of handled requests\n\
         # TYPE brrtrouter_requests_total counter\n\
         brrtrouter_requests_total {}\n\
         # HELP brrtrouter_request_latency_seconds Average request latency in seconds\n\
         # TYPE brrtrouter_request_latency_seconds gauge\n\
         brrtrouter_request_latency_seconds {}\n",
        metrics.request_count(),
        metrics.average_latency().as_secs_f64()
    );
    write_handler_response(res, 200, serde_json::Value::String(body), false, &HashMap::new());
    Ok(())
}

/// Streams the OpenAPI specification file as `text/yaml`.
pub fn openapi_endpoint(res: &mut Response, spec_path: &Path) -> io::Result<()> {
    match std::fs::read(spec_path) {
        Ok(bytes) => {
            res.status_code(200, "OK");
            res.header("Content-Type: text/yaml");
            res.body_vec(bytes);
        }
        Err(_) => {
            write_json_error(res, 404, serde_json::json!({ "error": "Spec not found" }));
        }
    }
    Ok(())
}

/// Serves the bundled Swagger UI `index.html`.
pub fn swagger_ui_endpoint(res: &mut Response) -> io::Result<()> {
    const INDEX: &str = include_str!("../../static/swagger-ui/index.html");
    res.status_code(200, "OK");
    res.header("Content-Type: text/html");
    res.body(INDEX);
    Ok(())
}

impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        let ParsedRequest {
            method,
            path,
            headers,
            cookies,
            query_params,
            body,
        } = parse_request(req);

        if method == "GET" && path == "/health" {
            return health_endpoint(res);
        }
        if method == "GET" && path == "/metrics" {
            if let Some(metrics) = &self.metrics {
                return metrics_endpoint(res, metrics);
            } else {
                write_json_error(
                    res,
                    404,
                    serde_json::json!({"error": "Not Found", "method": method, "path": path}),
                );
                return Ok(());
            }
        }
        if method == "GET" && path == "/openapi.yaml" {
            return openapi_endpoint(res, &self.spec_path);
        }
        if method == "GET" && path == "/docs" {
            return swagger_ui_endpoint(res);
        }

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
                            None => {
                                ok = false;
                                break;
                            }
                        };
                        let provider = match self.security_providers.get(scheme_name) {
                            Some(p) => p,
                            None => {
                                ok = false;
                                break;
                            }
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
                    write_json_error(res, 401, serde_json::json!({"error": "Unauthorized"}));
                    return Ok(());
                }
            }
            let is_sse = route_match.route.sse;
            let handler_response = {
                let dispatcher = self.dispatcher.read().unwrap();
                dispatcher.dispatch(route_match, body, headers, cookies)
            };
            match handler_response {
                Some(hr) => {
                    write_handler_response(res, hr.status, hr.body, is_sse, &hr.headers);
                }
                None => {
                    write_json_error(
                        res,
                        500,
                        serde_json::json!({
                            "error": "Handler failed or not registered",
                            "method": method,
                            "path": path
                        }),
                    );
                }
            }
        } else {
            write_json_error(
                res,
                404,
                serde_json::json!({"error": "Not Found", "method": method, "path": path}),
            );
        }
        Ok(())
    }
}
