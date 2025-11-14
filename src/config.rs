use serde::de::{Deserializer};
use serde::{Deserialize, Serialize};

/// Top-level configuration file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub applications: Vec<ApplicationConfig>,
}

/// Application configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationConfig {
    /// Logical name of the application
    pub name: String,

    /// Fetcher configuration
    pub fetcher: FetcherConfig,

    /// Installer configuration, supporting shorthand and explicit formats
    #[serde(deserialize_with = "deserialize_installer_config")]
    pub installer: InstallerConfig,

    /// Optional package name override for dpkg (defaults to `name` if not set)
    #[serde(default)]
    pub package_name: Option<String>,

    /// Optional flag to pin this application (no update checks)
    #[serde(default)]
    pub pinned: Option<bool>,
}

/// Configuration for different fetchers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherConfig {
    pub r#type: String,

    /// GitHub repo in the form "owner/repo", for GitHub fetcher
    #[serde(default)]
    pub repo: Option<String>,

    /// File pattern (glob) to match assets
    #[serde(default)]
    pub file_pattern: Option<String>,
}

/// Installer configuration.
///
/// This supports both:
///
/// ```yaml
/// installer:
///   type: deb
/// ```
///
/// and:
///
/// ```yaml
/// installer: deb
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerConfig {
    pub r#type: String,
}

/// Helper enum used for custom deserialization to support shorthand installer syntax.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum InstallerConfigIntermediate {
    String(String),
    Full { r#type: String },
}

fn deserialize_installer_config<'de, D>(deserializer: D) -> Result<InstallerConfig, D::Error>
where
    D: Deserializer<'de>,
{
    let intermediate = InstallerConfigIntermediate::deserialize(deserializer)?;
    match intermediate {
        InstallerConfigIntermediate::String(s) => Ok(InstallerConfig { r#type: s }),
        InstallerConfigIntermediate::Full { r#type } => Ok(InstallerConfig { r#type }),
    }
}
