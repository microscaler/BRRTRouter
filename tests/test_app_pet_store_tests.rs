//! Epic 13.10 — pet_store smoke via public `TestApp` (P5 / FR-1).
#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

mod tracing_util;

use arc_swap::ArcSwap;
use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::middleware::TracingMiddleware;
use brrtrouter::router::Router;
use brrtrouter::security::{SecurityProvider, SecurityRequest};
use brrtrouter::server::AppService;
use brrtrouter::spec::SecurityScheme;
use brrtrouter::test_support::{TestApp, TestAppOptions};
use pet_store::registry;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_util::TestTracing;

static HTTP_TEST_LOCK: Mutex<()> = Mutex::new(());

struct ApiKeyProvider {
    key: String,
}

impl SecurityProvider for ApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                "header" => req
                    .get_header(&name.to_ascii_lowercase())
                    .map(|v| v == self.key)
                    .unwrap_or(false),
                "query" => req.get_query(name).map(|v| v == self.key).unwrap_or(false),
                "cookie" => req.get_cookie(name).map(|v| v == self.key).unwrap_or(false),
                _ => false,
            },
            _ => false,
        }
    }
}

/// Build pet_store TestApp with API-key security matching config `test123`.
fn pet_store_app_with_api_key() -> (TestTracing, TestApp) {
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    let spec = "examples/pet_store/doc/openapi.yaml";
    let (routes, schemes, _) = brrtrouter::load_spec_full(spec).unwrap();
    let router = Arc::new(ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(ArcSwap::from_pointee(dispatcher)),
        schemes,
        PathBuf::from(spec),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    for (name, scheme) in service.security_schemes.clone() {
        if matches!(scheme, SecurityScheme::ApiKey { .. }) {
            service.register_security_provider(
                &name,
                Arc::new(ApiKeyProvider {
                    key: "test123".into(),
                }),
            );
        }
    }
    let app = TestApp::from_service(service).expect("pet_store TestApp starts");
    (tracing, app)
}

#[test]
fn p5_pet_store_list_pets_via_test_app() {
    let _lock = HTTP_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_tracing, app) = pet_store_app_with_api_key();
    let res = app
        .get("/pets")
        .header("X-API-Key", "test123")
        .send()
        .unwrap();
    assert_eq!(res.status, 200, "body={}", res.text().unwrap_or(""));
    let body = res.json_value().unwrap();
    assert!(
        body.is_array() || body.get("items").is_some() || body.is_object(),
        "unexpected pets payload: {body}"
    );
}

#[test]
fn p5_pet_store_from_spec_registers_handlers() {
    let _lock = HTTP_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    let spec = PathBuf::from("examples/pet_store/doc/openapi.yaml");
    let app = TestApp::from_spec_with_options(
        &spec,
        TestAppOptions {
            static_dir: Some(PathBuf::from("examples/pet_store/static_site")),
            doc_dir: Some(PathBuf::from("examples/pet_store/doc")),
        },
        |dispatcher, routes| unsafe {
            registry::register_from_spec(dispatcher, routes);
        },
    )
    .expect("from_spec starts");
    // Secured route without API key → 401 (proves routing + security wired).
    let res = app.get("/pets").send().unwrap();
    assert_eq!(res.status, 401, "body={}", res.text().unwrap_or(""));
}
