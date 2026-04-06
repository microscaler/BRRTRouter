#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::generator::FieldDef;
use brrtrouter::generator::{
    write_controller, write_handler, write_impl_controller_stub, write_impl_main_rs,
    write_main_rs, write_registry_rs, ImplControllerStubParams, RegistryEntry,
};
use brrtrouter::spec::{ParameterMeta, RouteMeta};
use http::Method;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("gen_tpl_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_template_writers() {
    let dir = temp_dir();
    let src_dir = dir.join("src");
    let handlers_dir = src_dir.join("handlers");
    let controllers_dir = src_dir.join("controllers");
    fs::create_dir_all(&handlers_dir).unwrap();
    fs::create_dir_all(&controllers_dir).unwrap();

    let req_fields = vec![FieldDef {
        name: "id".into(),
        original_name: "id".into(),
        ty: "String".into(),
        optional: false,
        value: "\"id\".to_string()".into(),
    }];
    let res_fields = vec![FieldDef {
        name: "ok".into(),
        original_name: "ok".into(),
        ty: "bool".into(),
        optional: false,
        value: "true".into(),
    }];
    let imports = BTreeSet::new();
    let params: Vec<ParameterMeta> = Vec::new();

    let handler_path = handlers_dir.join("test.rs");
    write_handler(
        &handler_path,
        "test",
        &req_fields,
        &res_fields,
        &imports,
        &params,
        false,
        true,
        true,
    )
    .unwrap();

    let controller_path = controllers_dir.join("test.rs");
    write_controller(
        &controller_path,
        "test",
        "TestController",
        &res_fields,
        None,
        false,
        true,
        None,
        None,
        "crate::AppState".to_string(),
    )
    .unwrap();

    let entries = vec![RegistryEntry {
        name: "test".into(),
        request_type: "test::Request".into(),
        controller_struct: "TestController".into(),
        parameters: vec![],
        stack_size_bytes: 16384,
        is_proxy: false,
    }];
    write_registry_rs(&src_dir, &entries).unwrap();

    let route = RouteMeta { x_service: None, x_brrtrouter_downstream_path: None,
        method: Method::GET,
        path_pattern: "/test".into(),
        handler_name: "test".into(),
        parameters: vec![],
        request_schema: None,
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: HashMap::new(),
        security: vec![],
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
        estimated_request_body_bytes: None,
        x_brrtrouter_stack_size: None,
        cors_policy: brrtrouter::middleware::RouteCorsPolicy::Inherit,
    };
    write_main_rs(&src_dir, "tester", vec![route]).unwrap();

    let main_content = fs::read_to_string(src_dir.join("main.rs")).unwrap();
    assert!(main_content.contains("fn main()"));

    let handler_content = fs::read_to_string(&handler_path).unwrap();
    assert!(handler_content.contains("pub struct Request"));
    assert!(handler_content.contains("pub struct Response"));

    let controller_content = fs::read_to_string(&controller_path).unwrap();
    assert!(controller_content.contains("#[handler(TestController)]"));

    let registry_content = fs::read_to_string(src_dir.join("registry.rs")).unwrap();
    assert!(registry_content.contains("pub unsafe fn register_all"));
    assert!(registry_content.contains("register_from_spec"));

    fs::remove_dir_all(&dir).unwrap();
}

/// Hyphenated Cargo package names must become valid Rust `use` paths (underscores).
#[test]
fn impl_main_rs_maps_hyphenated_package_to_rust_crate_ident() {
    let dir = temp_dir();
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    write_impl_main_rs(&src_dir, "market-data_service_api", &[]).unwrap();

    let main_content = fs::read_to_string(src_dir.join("main.rs")).unwrap();
    assert!(
        main_content.contains("use market_data_service_api::registry;"),
        "expected canonical Rust crate path in use line, got excerpt: {}",
        &main_content[..main_content.len().min(400)]
    );
    assert!(
        !main_content.contains("use market-data_service_api::"),
        "hyphenated `use` path is invalid Rust (parsed as subtraction)"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn impl_controller_stub_maps_hyphenated_package_in_use_lines() {
    let dir = temp_dir();
    let controllers_dir = dir.join("controllers");
    fs::create_dir_all(&controllers_dir).unwrap();

    let req_fields = vec![FieldDef {
        name: "id".into(),
        original_name: "id".into(),
        ty: "String".into(),
        optional: false,
        value: "\"x\".to_string()".into(),
    }];
    let res_fields = vec![FieldDef {
        name: "ok".into(),
        original_name: "ok".into(),
        ty: "bool".into(),
        optional: false,
        value: "true".into(),
    }];
    let stub_path = controllers_dir.join("get_items.rs");

    write_impl_controller_stub(ImplControllerStubParams {
        path: &stub_path,
        handler: "get_items",
        struct_name: "GetItemsController",
        crate_name: "market-data_service_api",
        req_fields: &req_fields,
        res_fields: &res_fields,
        imports: &BTreeSet::new(),
        sse: false,
        example: None,
        force: true,
    })
    .unwrap();

    let content = fs::read_to_string(&stub_path).unwrap();
    assert!(
        content.contains("use market_data_service_api::handlers::get_items::{Request, Response};"),
        "stub: {}",
        &content[..content.len().min(500)]
    );
    assert!(
        !content.contains("use market-data_service_api::"),
        "invalid hyphenated use path must not appear"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn impl_main_rs_plain_snake_package_unchanged_in_use_line() {
    let dir = temp_dir();
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    write_impl_main_rs(&src_dir, "amd_service_api", &[]).unwrap();

    let main_content = fs::read_to_string(src_dir.join("main.rs")).unwrap();
    assert!(main_content.contains("use amd_service_api::registry;"));

    fs::remove_dir_all(&dir).unwrap();
}
