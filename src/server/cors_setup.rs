//! CORS middleware construction at startup (JSF: no runtime config parsing in hot path).

use std::collections::HashMap;
use std::sync::Arc;

use http::Method;

use crate::middleware::{
    build_route_cors_map, CorsMiddleware, CorsMiddlewareBuilder, MetricsMiddleware,
    RouteCorsPolicy,
};
use crate::spec::RouteMeta;

use super::app_config::AppConfig;

/// Build route-aware CORS middleware from `config.yaml` + OpenAPI `x-cors` policies.
pub fn build_cors_middleware(
    app_config: &AppConfig,
    routes: &[RouteMeta],
    metrics: Arc<MetricsMiddleware>,
) -> Option<Arc<CorsMiddleware>> {
    let cors_cfg = app_config.cors.as_ref();
    let origins = cors_cfg
        .and_then(|c| c.origins.as_ref())
        .map(|o| o.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut builder = CorsMiddlewareBuilder::new();
    if !origins.is_empty() {
        builder = builder.allowed_origins(&origins);
    }

    if let Some(cfg) = cors_cfg {
        if let Some(headers) = cfg.allowed_headers.as_ref() {
            let header_strs: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
            builder = builder.allowed_headers(&header_strs);
        }
        if let Some(methods) = cfg.allowed_methods.as_ref() {
            let method_vec: Vec<Method> = methods
                .iter()
                .filter_map(|m| m.parse::<Method>().ok())
                .collect();
            if !method_vec.is_empty() {
                builder = builder.allowed_methods(&method_vec);
            }
        }
        if let Some(creds) = cfg.allow_credentials {
            builder = builder.allow_credentials(creds);
        }
        if let Some(expose) = cfg.expose_headers.as_ref() {
            let expose_strs: Vec<&str> = expose.iter().map(|s| s.as_str()).collect();
            builder = builder.expose_headers(&expose_strs);
        }
        if let Some(age) = cfg.max_age {
            builder = builder.max_age(age);
        }
    }

    match builder.build() {
        Ok(global_cors) => {
            let route_policies = build_route_cors_map(routes);
            let mut merged_policies = HashMap::new();
            for (handler_name, policy) in route_policies {
                let merged_policy = match policy {
                    RouteCorsPolicy::Custom(route_config) => {
                        RouteCorsPolicy::Custom(route_config.with_origins(&origins))
                    }
                    other => other,
                };
                merged_policies.insert(handler_name, merged_policy);
            }
            Some(Arc::new(
                CorsMiddleware::with_route_policies(global_cors, merged_policies)
                    .with_metrics_sink(metrics),
            ))
        }
        Err(e) => {
            eprintln!("[cors][error] Failed to build CORS middleware: {e:?}");
            None
        }
    }
}
