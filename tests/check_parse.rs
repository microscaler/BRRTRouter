#[test]
fn test_parse_bff() {
    let spec = oas3::from_path("/Users/casibbald/Workspace/hauliage/openapi/openapi_bff.yaml").unwrap();
    let op = spec.paths.as_ref().unwrap().get("/api/v1/consignments/ai/tidy-text").unwrap().post.as_ref().unwrap();
    println!("Extensions: {:?}", op.extensions);
    assert!(op.extensions.contains_key("x-service"), "x-service is missing in oas3 parser output!");
}
