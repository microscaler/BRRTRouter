use super::*;
use std::collections::HashSet;

#[test]
fn test_unique_handler_name() {
    let mut seen = HashSet::new();
    let a = unique_handler_name(&mut seen, "foo");
    assert_eq!(a, "foo");
    let b = unique_handler_name(&mut seen, "foo");
    assert_eq!(b, "foo_1");
    let c = unique_handler_name(&mut seen, "foo");
    assert_eq!(c, "foo_2");
}
