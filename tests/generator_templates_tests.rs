use brrtrouter::generator::{
    write_main_rs, write_handler, write_controller, write_registry_rs, RegistryEntry,
};
use brrtrouter::generator::FieldDef;
use brrtrouter::spec::{ParameterMeta, RouteMeta};
use http::Method;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{PathBuf};
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
        ty: "String".into(),
        optional: false,
        value: "\"id\".to_string()".into(),
    }];
    let res_fields = vec![FieldDef {
        name: "ok".into(),
        ty: "bool".into(),
        optional: false,
        value: "true".into(),
    }];
    let imports = BTreeSet::new();
    let params: Vec<ParameterMeta> = Vec::new();

    let handler_path = handlers_dir.join("test.rs");
    write_handler(&handler_path, "test", &req_fields, &res_fields, &imports, &params, true).unwrap();

    let controller_path = controllers_dir.join("test.rs");
    write_controller(&controller_path, "test", "TestController", &res_fields, None, true).unwrap();

    let entries = vec![RegistryEntry {
        name: "test".into(),
        request_type: "test::Request".into(),
        controller_struct: "TestController".into(),
        parameters: vec![],
    }];
    write_registry_rs(&src_dir, &entries).unwrap();

    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/test".into(),
        handler_name: "test".into(),
        parameters: vec![],
        request_schema: None,
        response_schema: None,
        example: None,
        responses: HashMap::new(),
        security: vec![],
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    write_main_rs(&src_dir, "tester", vec![route]).unwrap();

    let main_content = fs::read_to_string(src_dir.join("main.rs")).unwrap();
    assert!(main_content.contains("fn main()"));

    let handler_content = fs::read_to_string(&handler_path).unwrap();
    assert!(handler_content.contains("#[handler("));

    let controller_content = fs::read_to_string(&controller_path).unwrap();
    assert!(controller_content.contains("pub struct TestController"));
    assert!(controller_content.contains("impl Handler for TestController"));

    let registry_content = fs::read_to_string(src_dir.join("registry.rs")).unwrap();
    assert!(registry_content.contains("pub unsafe fn register_all"));
    assert!(registry_content.contains("register_from_spec"));

    fs::remove_dir_all(&dir).unwrap();
}
