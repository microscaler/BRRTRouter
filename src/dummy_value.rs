// dummy_value.rs

pub fn dummy_value(ty: &str) -> askama::Result<String> {
    let value = match ty {
        "String" => "\"example\".to_string()",
        "i32" => "42",
        "f64" => "3.14",
        "bool" => "true",
        "Vec<Value>" | "Vec<String>" | "Vec<i32>" | "Vec<f64>" | "Vec<bool>" => "vec![]",
        _ => "Default::default()",
    };
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::dummy_value;

    #[test]
    fn test_string() {
        assert_eq!(dummy_value("String").unwrap(), "\"example\".to_string()");
    }

    #[test]
    fn test_i32() {
        assert_eq!(dummy_value("i32").unwrap(), "42");
    }

    #[test]
    fn test_f64() {
        assert_eq!(dummy_value("f64").unwrap(), "3.14");
    }

    #[test]
    fn test_bool() {
        assert_eq!(dummy_value("bool").unwrap(), "true");
    }

    #[test]
    fn test_vec_string() {
        assert_eq!(dummy_value("Vec<String>").unwrap(), "vec![]");
    }

    #[test]
    fn test_vec_i32() {
        assert_eq!(dummy_value("Vec<i32>").unwrap(), "vec![]");
    }

    #[test]
    fn test_vec_f64() {
        assert_eq!(dummy_value("Vec<f64>").unwrap(), "vec![]");
    }

    #[test]
    fn test_vec_bool() {
        assert_eq!(dummy_value("Vec<bool>").unwrap(), "vec![]");
    }

    #[test]
    fn test_vec_value() {
        assert_eq!(dummy_value("Vec<Value>").unwrap(), "vec![]");
    }

    #[test]
    fn test_default() {
        assert_eq!(dummy_value("Other").unwrap(), "Default::default()");
    }
}
