use std::env;

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    pub stack_size: usize,
}

impl RuntimeConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        let stack_size = match env::var("BRRTR_STACK_SIZE") {
            Ok(val) => {
                if let Some(hex) = val.strip_prefix("0x") {
                    usize::from_str_radix(hex, 16).unwrap_or(0x4000)
                } else {
                    val.parse().unwrap_or(0x4000)
                }
            }
            Err(_) => 0x4000,
        };
        RuntimeConfig { stack_size }
    }
}
