use crate::config::ApplicationConfig;
use crate::installer::Installer;
use crate::types::UpdateCheck;

use anyhow::{anyhow, Context, Result};
use log::{info, warn};
use std::path::Path;
use std::process::Command;
use which::which;

/// Installer for Debian (.deb) packages.
pub struct DebInstaller {
    package_name: String,
    pinned: bool,
}

impl DebInstaller {
    pub fn new(app: &ApplicationConfig) -> Result<Self> {
        let package_name = app.package_name.clone().unwrap_or_else(|| app.name.clone());
        let pinned = app.pinned.unwrap_or(false);

        Ok(Self {
            package_name,
            pinned,
        })
    }

    fn get_installed_version(&self) -> Result<Option<String>> {
        // Uses "dpkg -s <package>" to get installed version
        if which("dpkg").is_err() {
            warn!("dpkg not found in PATH; cannot query installed version");
            return Ok(None);
        }

        let output = Command::new("dpkg")
            .arg("-s")
            .arg(&self.package_name)
            .output()
            .with_context(|| "Failed to run dpkg -s")?;

        if !output.status.success() {
            // Package likely not installed
            info!(
                "dpkg -s {} failed with status {}; assuming not installed",
                self.package_name, output.status
            );
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("Version:") {
                let version = rest.trim().to_string();
                info!(
                    "DebInstaller: found installed version for {}: {}",
                    self.package_name, version
                );
                return Ok(Some(version));
            }
        }

        Ok(None)
    }

    fn run_install_command(&self, file_path: &Path) -> Result<()> {
        // Prefer sudo dpkg -i <file>
        let cmd = if which("sudo").is_ok() {
            ("sudo", vec!["dpkg", "-i"])
        } else {
            ("dpkg", vec!["-i"])
        };

        let mut args: Vec<String> = cmd.1.iter().map(|s| s.to_string()).collect();
        args.push(file_path.display().to_string());

        info!(
            "Running install command: {} {}",
            cmd.0,
            args.join(" ")
        );

        let status = Command::new(cmd.0)
            .args(&args)
            .status()
            .with_context(|| "Failed to run installer command")?;

        if !status.success() {
            return Err(anyhow!("Installer command failed with status {}", status));
        }

        Ok(())
    }
}

impl Installer for DebInstaller {
    fn should_check_for_update(&self) -> Result<UpdateCheck> {
        if self.pinned {
            info!(
                "DebInstaller: package {} is pinned; skipping update check",
                self.package_name
            );
            return Ok(UpdateCheck::No);
        }

        match self.get_installed_version()? {
            Some(v) => Ok(UpdateCheck::Yes(v)),
            None => {
                // Treat "not installed" as version "0.0.0"
                info!(
                    "DebInstaller: package {} not installed; treating as version 0.0.0",
                    self.package_name
                );
                Ok(UpdateCheck::Yes("0.0.0".to_string()))
            }
        }
    }

    fn install(&self, file_path: &Path) -> Result<()> {
        self.run_install_command(file_path)
    }
}