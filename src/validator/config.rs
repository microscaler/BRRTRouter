use super::types::ParameterValidationConfig;
use std::collections::HashMap;

/// Main validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub strict_mode: bool,
    pub max_request_size: usize,
    pub validate_responses: bool,
    pub development_mode: bool,
    pub parameter_validation: ParameterValidationConfig,
    pub cache_size: usize,
    pub cache_ttl_seconds: u64,
    pub per_operation_overrides: HashMap<String, OperationValidationConfig>,
}

/// Per-operation validation configuration
#[derive(Debug, Clone)]
pub struct OperationValidationConfig {
    pub strict_mode: Option<bool>,
    pub max_request_size: Option<usize>,
    pub validate_responses: Option<bool>,
    pub parameter_validation: Option<ParameterValidationConfig>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            strict_mode: true,
            max_request_size: 10 * 1024 * 1024, // 10MB
            validate_responses: true,
            development_mode: false,
            parameter_validation: ParameterValidationConfig::default(),
            cache_size: 1000,
            cache_ttl_seconds: 3600, // 1 hour
            per_operation_overrides: HashMap::new(),
        }
    }
}

impl ValidationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    pub fn with_max_request_size(mut self, size: usize) -> Self {
        self.max_request_size = size;
        self
    }

    pub fn with_development_mode(mut self, dev_mode: bool) -> Self {
        self.development_mode = dev_mode;
        self
    }

    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = size;
        self
    }

    pub fn add_operation_override(
        mut self,
        operation_id: impl Into<String>,
        config: OperationValidationConfig,
    ) -> Self {
        self.per_operation_overrides.insert(operation_id.into(), config);
        self
    }

    /// Get effective configuration for a specific operation
    pub fn for_operation(&self, operation_id: &str) -> ValidationConfig {
        let mut config = self.clone();
        
        if let Some(override_config) = self.per_operation_overrides.get(operation_id) {
            if let Some(strict) = override_config.strict_mode {
                config.strict_mode = strict;
            }
            if let Some(size) = override_config.max_request_size {
                config.max_request_size = size;
            }
            if let Some(validate_resp) = override_config.validate_responses {
                config.validate_responses = validate_resp;
            }
            if let Some(param_config) = &override_config.parameter_validation {
                config.parameter_validation = param_config.clone();
            }
        }
        
        config
    }
}

impl OperationValidationConfig {
    pub fn new() -> Self {
        Self {
            strict_mode: None,
            max_request_size: None,
            validate_responses: None,
            parameter_validation: None,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = Some(strict);
        self
    }

    pub fn with_max_request_size(mut self, size: usize) -> Self {
        self.max_request_size = Some(size);
        self
    }

    pub fn with_response_validation(mut self, validate: bool) -> Self {
        self.validate_responses = Some(validate);
        self
    }
}

impl Default for OperationValidationConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment-based configuration loader
impl ValidationConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(strict) = std::env::var("BRRTR_VALIDATION_STRICT") {
            config.strict_mode = strict.parse().unwrap_or(true);
        }

        if let Ok(size) = std::env::var("BRRTR_MAX_REQUEST_SIZE") {
            config.max_request_size = size.parse().unwrap_or(10 * 1024 * 1024);
        }

        if let Ok(dev_mode) = std::env::var("BRRTR_DEVELOPMENT_MODE") {
            config.development_mode = dev_mode.parse().unwrap_or(false);
        }

        if let Ok(validate_resp) = std::env::var("BRRTR_VALIDATE_RESPONSES") {
            config.validate_responses = validate_resp.parse().unwrap_or(true);
        }

        if let Ok(cache_size) = std::env::var("BRRTR_VALIDATION_CACHE_SIZE") {
            config.cache_size = cache_size.parse().unwrap_or(1000);
        }

        config
    }
} 