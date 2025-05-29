use super::Router;

#[test]
fn test_root_path() {
    let (re, params) = Router::path_to_regex("/");
    assert!(re.is_match("/"));
    assert!(params.is_empty());
}

#[test]
fn test_parameterized_path() {
    let (re, params) = Router::path_to_regex("/items/{id}");
    assert!(re.is_match("/items/123"));
    assert_eq!(params, vec!["id"]);
}

#[test]
fn test_nested_path() {
    let (re, params) = Router::path_to_regex("/a/{b}/c");
    assert!(re.is_match("/a/1/c"));
    assert_eq!(params, vec!["b"]);
}
