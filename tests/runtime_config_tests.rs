use brrtrouter::runtime_config::RuntimeConfig;
use std::env;

#[test]
fn test_default_stack_size() {
    // Remove environment variable if present
    env::remove_var("BRRTR_STACK_SIZE");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0x4000);
}

#[test]
fn test_decimal_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "32768");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 32768);
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_hex_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "0x8000");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0x8000);
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_invalid_decimal_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "invalid");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0x4000); // Should fall back to default
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_invalid_hex_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "0xinvalid");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0x4000); // Should fall back to default
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_empty_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0x4000); // Should fall back to default
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_zero_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "0");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0);
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_large_stack_size() {
    env::set_var("BRRTR_STACK_SIZE", "1048576"); // 1MB
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 1048576);
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_hex_prefix_only() {
    env::set_var("BRRTR_STACK_SIZE", "0x");
    
    let config = RuntimeConfig::from_env();
    assert_eq!(config.stack_size, 0x4000); // Should fall back to default
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_runtime_config_debug() {
    let config = RuntimeConfig { stack_size: 0x8000 };
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("RuntimeConfig"));
    assert!(debug_str.contains("stack_size"));
    assert!(debug_str.contains("32768"));
}

#[test]
fn test_runtime_config_clone() {
    let config1 = RuntimeConfig { stack_size: 0x8000 };
    let config2 = config1.clone();
    assert_eq!(config1.stack_size, config2.stack_size);
}

#[test]
fn test_runtime_config_copy() {
    let config1 = RuntimeConfig { stack_size: 0x8000 };
    let config2 = config1; // Copy semantics due to Copy trait
    assert_eq!(config1.stack_size, config2.stack_size);
    // Both should still be usable
    assert_eq!(config1.stack_size, 0x8000);
    assert_eq!(config2.stack_size, 0x8000);
}

#[test]
fn test_multiple_environment_calls() {
    // Test that multiple calls to from_env() are consistent
    env::set_var("BRRTR_STACK_SIZE", "0x10000");
    
    let config1 = RuntimeConfig::from_env();
    let config2 = RuntimeConfig::from_env();
    
    assert_eq!(config1.stack_size, config2.stack_size);
    assert_eq!(config1.stack_size, 0x10000);
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
}

#[test]
fn test_may_coroutines_stack_sizes() {
    // Test stack sizes that are commonly used with May coroutines
    let test_cases = vec![
        ("4096", 4096),           // 4KB - minimum practical
        ("8192", 8192),           // 8KB - small stack
        ("16384", 16384),         // 16KB - default
        ("65536", 65536),         // 64KB - large stack
        ("0x1000", 0x1000),       // 4KB in hex
        ("0x4000", 0x4000),       // 16KB in hex
        ("0x10000", 0x10000),     // 64KB in hex
    ];
    
    for (env_val, expected) in test_cases {
        env::set_var("BRRTR_STACK_SIZE", env_val);
        let config = RuntimeConfig::from_env();
        assert_eq!(config.stack_size, expected, "Failed for input: {}", env_val);
    }
    
    // Cleanup
    env::remove_var("BRRTR_STACK_SIZE");
} 