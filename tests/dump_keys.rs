#[test]
fn test_dump() {
    let spec = oas3::from_path("/Users/casibbald/Workspace/hauliage/openapi/openapi_bff.yaml").unwrap();
    let op = spec.paths.as_ref().unwrap().get("/api/v1/consignments/ai/tidy-text").unwrap().post.as_ref().unwrap();
    println!("EXTENSIONS DICT: {:?}", op.extensions.keys().collect::<Vec<_>>());
}
