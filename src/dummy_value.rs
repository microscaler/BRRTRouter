// dummy_value.rs
use askama::Template;

pub fn dummy_value(ty: &str) -> askama::Result<String> {
    let value = match ty {
        "String" => "\"example\".to_string()",
        "i32" => "42",
        "f64" => "3.14",
        "bool" => "true",
        "Vec<Value>" => "vec![]",
        _ => "Default::default()",
    };
    Ok(value.to_string())
}
