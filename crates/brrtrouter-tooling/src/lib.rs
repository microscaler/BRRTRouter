pub mod discovery;
pub mod scan;
pub mod ci;
pub mod paths;
pub mod docker;
pub mod build;
pub mod gen;

/// Result type alias for tooling operations
pub type ToolingResult<T = ()> = anyhow::Result<T>;
