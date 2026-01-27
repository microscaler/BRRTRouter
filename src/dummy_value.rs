// dummy_value.rs

pub fn dummy_value(ty: &str) -> askama::Result<String> {
    let value = match ty {
        "String" => "\"example\".to_string()".to_string(),
        "i32" => "42".to_string(),
        "f64" => "3.14".to_string(),  // Valid mathematical number - clippy warning is acceptable for f64
        "rust_decimal::Decimal" => {
            // General decimal: 123.45
            "rust_decimal::Decimal::new(12345, 2)".to_string()
        }
        "rusty_money::Money" => {
            // Money: $3.14 USD - clearly a dollar amount, not PI
            // from_minor(314, USD) = 314 cents = $3.14
            "rusty_money::Money::from_minor(314, rusty_money::iso::USD)".to_string()
        }
        "bool" => "true".to_string(),
        "Vec<Value>" | "Vec<String>" | "Vec<i32>" | "Vec<f64>" | "Vec<bool>" => "vec![]".to_string(),
        _ => "Default::default()".to_string(),
    };
    Ok(value)
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

    #[test]
    fn test_rust_decimal() {
        assert_eq!(
            dummy_value("rust_decimal::Decimal").unwrap(),
            "rust_decimal::Decimal::new(12345, 2)"
        );
    }

    #[test]
    fn test_rusty_money_usd() {
        // Test that Money type returns $3.14 USD (314 cents)
        let result = dummy_value("rusty_money::Money").unwrap();
        assert!(result.contains("rusty_money::Money::from_minor"));
        assert!(result.contains("314"));
        assert!(result.contains("rusty_money::iso::USD"));
        // Verify it's exactly $3.14 (314 cents)
        assert_eq!(
            result,
            "rusty_money::Money::from_minor(314, rusty_money::iso::USD)"
        );
    }

    #[test]
    fn test_rusty_money_contains_314() {
        // Verify that Money dummy value uses 314 (cents) which equals $3.14
        // This ensures we're using $3.14 and not just any value
        let result = dummy_value("rusty_money::Money").unwrap();
        assert!(result.contains("314"), "Money dummy value should use 314 cents ($3.14)");
    }

    #[test]
    fn test_rusty_money_currency_iso() {
        // Verify that Money uses ISO currency format
        let result = dummy_value("rusty_money::Money").unwrap();
        assert!(
            result.contains("rusty_money::iso::USD"),
            "Money should use ISO currency format"
        );
    }
}
