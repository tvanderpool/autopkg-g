pub mod deb;

use crate::config::{ApplicationConfig, InstallerConfig};
use crate::types::UpdateCheck;
use anyhow::{anyhow, Result};

/// Trait for installing updates.
pub trait Installer {
    /// Decide whether an update check should occur, and report current version if so.
    fn should_check_for_update(&self) -> Result<UpdateCheck>;

    /// Install the file at `file_path`.
    fn install(&self, file_path: &std::path::Path) -> Result<()>;
}

/// Factory for installers.
pub fn create_installer(
    config: &InstallerConfig,
    app: &ApplicationConfig,
) -> Result<Box<dyn Installer>> {
    match config.r#type.as_str() {
        "deb" => Ok(Box::new(deb::DebInstaller::new(app)?)),
        other => Err(anyhow!("Unknown installer type: {}", other)),
    }
}
