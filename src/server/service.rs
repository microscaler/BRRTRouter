use super::request::{parse_request, ParsedRequest};
use super::response::{write_handler_response, write_json_error};
use crate::dispatcher::Dispatcher;
use crate::middleware::MetricsMiddleware;
use crate::router::Router;
use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use crate::static_files::StaticFiles;
use jsonschema::JSONSchema;
use may_minihttp::{HttpService, Request, Response};
use serde_json::json;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

pub struct AppService {
    pub router: Arc<RwLock<Router>>,
    pub dispatcher: Arc<RwLock<Dispatcher>>,
    pub security_schemes: HashMap<String, SecurityScheme>,
    pub security_providers: HashMap<String, Arc<dyn SecurityProvider>>,
    pub metrics: Option<Arc<crate::middleware::MetricsMiddleware>>,
    pub spec_path: PathBuf,
    pub static_files: Option<StaticFiles>,
    pub doc_files: Option<StaticFiles>,
    pub watcher: Option<notify::RecommendedWatcher>,
}

impl Clone for AppService {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            dispatcher: self.dispatcher.clone(),
            security_schemes: self.security_schemes.clone(),
            security_providers: self.security_providers.clone(),
            metrics: self.metrics.clone(),
            spec_path: self.spec_path.clone(),
            static_files: self.static_files.clone(),
            doc_files: self.doc_files.clone(),
            watcher: None,
        }
    }
}

impl AppService {
    pub fn new(
        router: Arc<RwLock<Router>>,
        dispatcher: Arc<RwLock<Dispatcher>>,
        security_schemes: HashMap<String, SecurityScheme>,
        spec_path: PathBuf,
        static_dir: Option<PathBuf>,
        doc_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            router,
            dispatcher,
            security_schemes,
            security_providers: HashMap::new(),
            metrics: None,
            spec_path,
            static_files: static_dir.map(StaticFiles::new),
            doc_files: doc_dir.map(StaticFiles::new),
            watcher: None,
        }
    }

    pub fn register_security_provider(&mut self, name: &str, provider: Arc<dyn SecurityProvider>) {
        self.security_providers.insert(name.to_string(), provider);
    }

    pub fn set_metrics_middleware(&mut self, metrics: Arc<MetricsMiddleware>) {
        self.metrics = Some(metrics);
    }

    /// Register default security providers based on loaded OpenAPI security schemes.
    ///
    /// This wires ApiKey, Bearer, and OAuth2 providers using environment variables or a
    /// provided test key for development. For ApiKey schemes, the following configuration
    /// is used (in order): per-scheme env `BRRTR_API_KEY__<SCHEME_NAME>`, global env
    /// `BRRTR_API_KEY`, then `test_api_key` argument, then fallback `"test123"`.
    pub fn register_default_security_providers_from_env(&mut self, test_api_key: Option<String>) {
        use std::sync::Arc as SyncArc;

        struct ApiKeyProvider {
            key: String,
        }
        impl SecurityProvider for ApiKeyProvider {
            fn validate(
                &self,
                scheme: &SecurityScheme,
                _scopes: &[String],
                req: &SecurityRequest,
            ) -> bool {
                match scheme {
                    SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                        "header" => {
                            // Accept either the named header or Authorization: Bearer <key> for migration convenience
                            let header_ok =
                                req.headers.get(&name.to_ascii_lowercase()) == Some(&self.key);
                            let auth_ok = req
                                .headers
                                .get("authorization")
                                .and_then(|h| h.strip_prefix("Bearer "))
                                .map(|v| v == self.key)
                                .unwrap_or(false);
                            header_ok || auth_ok
                        }
                        "query" => req.query.get(name) == Some(&self.key),
                        "cookie" => req.cookies.get(name) == Some(&self.key),
                        _ => false,
                    },
                    _ => false,
                }
            }
        }

        for (scheme_name, scheme) in self.security_schemes.clone() {
            match scheme {
                SecurityScheme::ApiKey { .. } => {
                    // Per-scheme env: BRRTR_API_KEY__<SCHEME_NAME>
                    let env_key_name = format!(
                        "BRRTR_API_KEY__{}",
                        scheme_name
                            .chars()
                            .map(|c| if c.is_ascii_alphanumeric() {
                                c.to_ascii_uppercase()
                            } else {
                                '_'
                            })
                            .collect::<String>()
                    );
                    let key = std::env::var(&env_key_name)
                        .ok()
                        .or_else(|| std::env::var("BRRTR_API_KEY").ok())
                        .or_else(|| test_api_key.clone())
                        .unwrap_or_else(|| "test123".to_string());
                    self.register_security_provider(
                        &scheme_name,
                        SyncArc::new(ApiKeyProvider { key }),
                    );
                }
                SecurityScheme::Http { ref scheme, .. }
                    if scheme.eq_ignore_ascii_case("bearer") =>
                {
                    // Simple development bearer provider; real validation can be plugged in by user
                    let provider = crate::security::BearerJwtProvider::new(
                        std::env::var("BRRTR_BEARER_SIGNATURE").unwrap_or_else(|_| "sig".into()),
                    );
                    self.register_security_provider(&scheme_name, SyncArc::new(provider));
                }
                SecurityScheme::OAuth2 { .. } => {
                    let provider = crate::security::OAuth2Provider::new(
                        std::env::var("BRRTR_OAUTH2_SIGNATURE").unwrap_or_else(|_| "sig".into()),
                    );
                    self.register_security_provider(&scheme_name, SyncArc::new(provider));
                }
                _ => {}
            }
        }
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
    let (stack_size, used_stack) = metrics.stack_usage();
    let body = format!(
        "# HELP brrtrouter_requests_total Total number of handled requests\n\
         # TYPE brrtrouter_requests_total counter\n\
         brrtrouter_requests_total {}\n\
         # HELP brrtrouter_top_level_requests_total Total number of received requests\n\
         # TYPE brrtrouter_top_level_requests_total counter\n\
         brrtrouter_top_level_requests_total {}\n\
         # HELP brrtrouter_auth_failures_total Total number of authentication failures\n\
         # TYPE brrtrouter_auth_failures_total counter\n\
         brrtrouter_auth_failures_total {}\n\
         # HELP brrtrouter_request_latency_seconds Average request latency in seconds\n\
         # TYPE brrtrouter_request_latency_seconds gauge\n\
         brrtrouter_request_latency_seconds {}\n\
         # HELP brrtrouter_coroutine_stack_bytes Configured coroutine stack size\n\
         # TYPE brrtrouter_coroutine_stack_bytes gauge\n\
         brrtrouter_coroutine_stack_bytes {}\n\
         # HELP brrtrouter_coroutine_stack_used_bytes Coroutine stack bytes used\n\
         # TYPE brrtrouter_coroutine_stack_used_bytes gauge\n\
         brrtrouter_coroutine_stack_used_bytes {}\n",
        metrics.request_count(),
        metrics.top_level_request_count(),
        metrics.auth_failures(),
        metrics.average_latency().as_secs_f64(),
        stack_size,
        used_stack
    );
    write_handler_response(
        res,
        200,
        serde_json::Value::String(body),
        false,
        &HashMap::new(),
    );
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

/// Serves the Swagger UI `index.html` from the configured docs directory.
pub fn swagger_ui_endpoint(res: &mut Response, docs: &StaticFiles) -> io::Result<()> {
    match docs.load("index.html", Some(&json!({ "spec_url": "/openapi.yaml" }))) {
        Ok((bytes, _)) => {
            res.status_code(200, "OK");
            res.header("Content-Type: text/html");
            res.body_vec(bytes);
        }
        Err(_) => {
            write_json_error(res, 404, serde_json::json!({ "error": "Docs not found" }));
        }
    }
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

        // Count every incoming request at top-level (even those short-circuited before dispatch)
        if let Some(metrics) = &self.metrics {
            metrics.inc_top_level_request();
        }

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
            if let Some(docs) = &self.doc_files {
                return swagger_ui_endpoint(res, docs);
            } else {
                write_json_error(
                    res,
                    404,
                    serde_json::json!({ "error": "Docs not configured" }),
                );
                return Ok(());
            }
        }

        if method == "GET" {
            if let Some(sf) = &self.static_files {
                let p = path.trim_start_matches('/');
                let p = if p.is_empty() { "index.html" } else { p };
                if let Ok((bytes, ct)) = sf.load(p, None) {
                    res.status_code(200, "OK");
                    let header = format!("Content-Type: {ct}").into_boxed_str();
                    res.header(Box::leak(header));
                    res.body_vec(bytes);
                    return Ok(());
                }
            }
        }

        let route_opt = {
            let router = self.router.read().unwrap();
            router.route(method.parse().unwrap(), &path)
        };
        if let Some(mut route_match) = route_opt {
            route_match.query_params = query_params.clone();
            // Perform security validation first
            if !route_match.route.security.is_empty() {
                let sec_req = SecurityRequest {
                    headers: &headers,
                    query: &query_params,
                    cookies: &cookies,
                };
                let mut authorized = false;
                let mut insufficient_scope = false;
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
                            // Detect insufficient scope for Bearer/OAuth2: token valid but scopes missing
                            match scheme {
                                SecurityScheme::Http {
                                    scheme: http_scheme,
                                    ..
                                } if http_scheme.eq_ignore_ascii_case("bearer") => {
                                    if provider.validate(scheme, &[], &sec_req) {
                                        insufficient_scope = true;
                                    }
                                }
                                SecurityScheme::OAuth2 { .. } => {
                                    if provider.validate(scheme, &[], &sec_req) {
                                        insufficient_scope = true;
                                    }
                                }
                                _ => {}
                            }
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
                    if let Some(metrics) = &self.metrics {
                        metrics.inc_auth_failure();
                    }
                    let debug = std::env::var("BRRTR_DEBUG_VALIDATION")
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false);
                    let status = if insufficient_scope { 403 } else { 401 };
                    let title = if status == 403 {
                        "Forbidden"
                    } else {
                        "Unauthorized"
                    };
                    let detail = if status == 403 {
                        "Insufficient scope or permissions"
                    } else {
                        "Missing or invalid credentials"
                    };
                    if status == 401 {
                        res.header("WWW-Authenticate: Bearer error=\"invalid_token\"");
                    } else {
                        res.header("WWW-Authenticate: Bearer error=\"insufficient_scope\"");
                    }
                    let mut body = serde_json::json!({
                        "type": "about:blank",
                        "title": title,
                        "status": status,
                        "detail": detail
                    });
                    if debug {
                        if let Some(map) = body.as_object_mut() {
                            map.insert("method".to_string(), json!(method));
                            map.insert("path".to_string(), json!(path));
                            map.insert(
                                "handler".to_string(),
                                json!(route_match.route.handler_name.clone()),
                            );
                        }
                    }
                    if status == 401 {
                        warn!(method=%method, path=%path, handler=%route_match.route.handler_name, "auth failed: 401 unauthorized");
                    } else {
                        warn!(method=%method, path=%path, handler=%route_match.route.handler_name, "auth failed: 403 forbidden");
                    }
                    write_json_error(res, status as u16, body);
                    return Ok(());
                }
                info!(method=%method, path=%path, handler=%route_match.route.handler_name, "auth success");
            }
            // Enforce required request body when specified in spec
            if route_match.route.request_body_required && body.is_none() {
                write_json_error(res, 400, json!({"error": "Request body required"}));
                return Ok(());
            }
            if let (Some(schema), Some(body_val)) = (&route_match.route.request_schema, &body) {
                let compiled = JSONSchema::compile(schema).expect("invalid request schema");
                let validation = compiled.validate(body_val);
                if let Err(errors) = validation {
                    let details: Vec<String> = errors.map(|e| e.to_string()).collect();
                    write_json_error(
                        res,
                        400,
                        json!({"error": "Request validation failed", "details": details}),
                    );
                    return Ok(());
                }
            }
            let is_sse = route_match.route.sse;
            let handler_response = {
                let dispatcher = self.dispatcher.read().unwrap();
                dispatcher.dispatch(route_match.clone(), body, headers, cookies)
            };
            match handler_response {
                Some(hr) => {
                    let mut headers = hr.headers.clone();
                    if !headers.contains_key("Content-Type") {
                        if let Some(ct) = route_match.route.content_type_for(hr.status) {
                            headers.insert("Content-Type".to_string(), ct);
                        }
                    }
                    if let Some(schema) = &route_match.route.response_schema {
                        let compiled =
                            JSONSchema::compile(schema).expect("invalid response schema");
                        let validation = compiled.validate(&hr.body);
                        if let Err(errors) = validation {
                            let details: Vec<String> = errors.map(|e| e.to_string()).collect();
                            write_json_error(
                                res,
                                400,
                                json!({"error": "Response validation failed", "details": details}),
                            );
                            return Ok(());
                        }
                    }
                    write_handler_response(res, hr.status, hr.body, is_sse, &headers);
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
