pub mod build;
pub mod ci;
pub mod discovery;
pub mod docker;
#[cfg(test)]
pub mod env_lock;
pub mod gen;
pub mod paths;
pub mod scan;

/// Result type alias for tooling operations
pub type ToolingResult<T = ()> = anyhow::Result<T>;
