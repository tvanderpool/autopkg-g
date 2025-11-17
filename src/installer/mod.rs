pub mod deb;

use crate::config::{ApplicationConfig, InstallerConfig};
use crate::types::UpdateCheck;
use anyhow::{Context, Result, anyhow};
use std::fmt::Display;
use std::io::{IsTerminal};
use std::process::{Command, ExitStatus};

/// Trait for installing updates.
pub trait Installer {
    /// Decide whether an update check should occur, and report current version if so.
    fn should_check_for_update(&self) -> Result<UpdateCheck>;

    /// Install the file at `file_path`.
    fn install(&self, file_path: &std::path::Path) -> Result<()>;
}

/// Check if sudo is needed and available.
/// Returns Ok(true) if running as root (sudo not needed).
/// Returns Ok(false) if not root but sudo is available and terminal is present.
/// Returns Err if not root and either sudo is unavailable or no terminal is available.
pub fn check_sudo_availability() -> Result<bool> {
    // Check if running as root
    let uid = nix::unistd::getuid();
    if uid.is_root() {
        return Ok(true); // Running as root, sudo not needed
    }

    // Not running as root, check if sudo is available
    if which::which("sudo").is_err() {
        return Err(anyhow!("Not running as root and sudo is not available in PATH"));
    }

    // Check if terminal is available for sudo password prompt
    if !std::io::stdin().is_terminal() {
        return Err(anyhow!(
            "Not running as root and no terminal available for sudo password prompt"
        ));
    }

    Ok(false) // Sudo needed and available
}

pub fn run_as_root<C, F>(cmd_args: &[&str], context: F) -> Result<ExitStatus>
where
    C: Display + Send + Sync + 'static,
    F: FnOnce() -> C,
{
    if !check_sudo_availability()? {
        // Run as root
        Command::new("sudo")
            .args(cmd_args)
            .status()
            .with_context(context)
    } else {
        // Already root, no sudo needed
        Command::new(cmd_args[0])
            .args(&cmd_args[1..])
            .status()
            .with_context(context)
    }
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
