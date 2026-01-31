//! Dependency configuration for BRRTRouter code generation
//!
//! Allows microservices to specify additional dependencies via a TOML config file
//! that sits alongside the OpenAPI spec.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dependency specification for Cargo.toml
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Simple version string: "1.33"
    Version(String),
    /// Workspace dependency: { workspace = true }
    Workspace {
        workspace: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        features: Option<Vec<String>>,
    },
    /// Full specification: { version = "1.33", features = ["serde"] }
    Full {
        version: Option<String>,
        path: Option<String>,
        git: Option<String>,
        branch: Option<String>,
        features: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<bool>,
    },
}

/// Dependency configuration loaded from brrtrouter-dependencies.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependenciesConfig {
    /// Dependencies to always include in generated Cargo.toml
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,

    /// Conditional dependencies - included if types are detected
    #[serde(default)]
    pub conditional: HashMap<String, ConditionalDependency>,
}

/// Conditional dependency that is included when a type pattern is detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalDependency {
    /// Type pattern to detect (e.g., "rust_decimal::Decimal")
    pub detect: String,
    /// Workspace dependency flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<bool>,
    /// Version string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Path to dependency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Git repository URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    /// Git branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Features to enable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
}

impl ConditionalDependency {
    /// Convert to DependencySpec
    pub fn to_spec(&self) -> DependencySpec {
        if let Some(true) = self.workspace {
            DependencySpec::Workspace {
                workspace: true,
                features: self.features.clone(),
            }
        } else if let Some(version) = &self.version {
            DependencySpec::Full {
                version: Some(version.clone()),
                path: self.path.clone(),
                git: self.git.clone(),
                branch: self.branch.clone(),
                features: self.features.clone(),
                workspace: self.workspace,
            }
        } else {
            // Default to workspace if nothing specified
            DependencySpec::Workspace {
                workspace: true,
                features: self.features.clone(),
            }
        }
    }
}

/// Load dependencies configuration from a TOML file
///
/// # Arguments
///
/// * `config_path` - Path to the brrtrouter-dependencies.toml file
///
/// # Returns
///
/// Returns `Ok(Some(config))` if file exists and parses successfully,
/// `Ok(None)` if file doesn't exist (not an error),
/// `Err` if file exists but fails to parse.
pub fn load_dependencies_config(config_path: &Path) -> anyhow::Result<Option<DependenciesConfig>> {
    if !config_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(config_path).with_context(|| {
        format!(
            "Failed to read dependencies config: {}",
            config_path.display()
        )
    })?;

    let config: DependenciesConfig = toml::from_str(&contents).with_context(|| {
        format!(
            "Failed to parse dependencies config: {}",
            config_path.display()
        )
    })?;

    Ok(Some(config))
}

/// Auto-detect dependencies config file alongside OpenAPI spec
///
/// Looks for `brrtrouter-dependencies.toml` in the same directory as the spec.
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI specification file
///
/// # Returns
///
/// Path to the config file if it exists, None otherwise.
pub fn auto_detect_config_path(spec_path: &Path) -> Option<PathBuf> {
    let spec_dir = spec_path.parent()?;
    let config_path = spec_dir.join("brrtrouter-dependencies.toml");
    if config_path.exists() {
        Some(config_path)
    } else {
        None
    }
}

/// Resolve dependencies config path
///
/// Priority:
/// 1. Explicitly provided path (via CLI)
/// 2. Auto-detected alongside spec
/// 3. None (no config)
pub fn resolve_config_path(explicit_path: Option<&Path>, spec_path: &Path) -> Option<PathBuf> {
    if let Some(path) = explicit_path {
        if path.exists() {
            return Some(path.to_path_buf());
        }
    }

    auto_detect_config_path(spec_path)
}

/// Path to the default brrtrouter-dependencies.toml alongside the spec (whether or not it exists).
pub fn default_config_path(spec_path: &Path) -> Option<PathBuf> {
    spec_path
        .parent()
        .map(|p| p.join("brrtrouter-dependencies.toml"))
}

/// Write brrtrouter-dependencies.toml starter content only if the file does not exist.
///
/// Used when the OpenAPI spec uses decimal/money and no config exists; the caller
/// provides content (e.g. from the Askama template). Does nothing if the file
/// already exists (does not overwrite).
pub fn write_dependencies_config_if_missing(
    config_path: &Path,
    content: &str,
) -> anyhow::Result<()> {
    if config_path.exists() {
        return Ok(());
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory for {}", config_path.display()))?;
    }
    std::fs::write(config_path, content).with_context(|| {
        format!(
            "Failed to write dependencies config: {}",
            config_path.display()
        )
    })?;
    Ok(())
}
