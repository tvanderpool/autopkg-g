mod config;
mod fetcher;
mod installer;
mod types;

use crate::config::Config;
use crate::fetcher::{create_fetcher, Fetcher};
use crate::installer::{create_installer, Installer};
use crate::types::UpdateCheck;

use anyhow::{Context, Result};
use clap::Parser;
use log::{error, info, warn};
use std::fs;
use std::path::PathBuf;

/// Auto-updater tool for applications defined in a YAML config.
#[derive(Parser, Debug)]
#[command(name = "autopkg")]
#[command(author, version, about)]
struct Cli {
    /// Path to config file (default: autopkg.yml in current directory)
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Check for updates without installing
    #[arg(long)]
    dry_run: bool,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, value_name = "LEVEL", default_value = "info")]
    log_level: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger to stderr
    std::env::set_var(
        "RUST_LOG",
        std::env::var("RUST_LOG").unwrap_or_else(|_| format!("autopkg={}", cli.log_level)),
    );
    env_logger::builder()
        .format_target(false)
        .format_timestamp_secs()
        .target(env_logger::Target::Stderr)
        .init();

    let config_path = cli
        .config
        .unwrap_or_else(|| PathBuf::from("autopkg.yml"));

    info!("Using config file: {}", config_path.display());

    let config_contents =
        fs::read_to_string(&config_path).with_context(|| "Failed to read config file")?;
    let config: Config =
        serde_yaml::from_str(&config_contents).with_context(|| "Failed to parse config YAML")?;

    info!("Loaded {} application(s) from config", config.applications.len());

    for app in &config.applications {
        info!("Processing application: {}", app.name);

        if let Err(e) = process_application(app, cli.dry_run) {
            error!(
                "Application '{}' failed: {:?}. Continuing with others.",
                app.name, e
            );
        }
    }

    Ok(())
}

fn process_application(app: &config::ApplicationConfig, dry_run: bool) -> Result<()> {
    let installer = create_installer(&app.installer, app)?;
    let fetcher = create_fetcher(&app.fetcher, app)?;

    match installer.should_check_for_update()? {
        UpdateCheck::No => {
            info!("{}: update check skipped (pinned or disabled)", app.name);
        }
        UpdateCheck::Yes(current_version) => {
            info!(
                "{}: current version reported by installer: {}",
                app.name, current_version
            );

            match fetcher.fetch_if_newer(&current_version)? {
                None => {
                    info!("{}: already up-to-date", app.name);
                }
                Some(downloaded_path) => {
                    if dry_run {
                        warn!(
                            "{}: update available (downloaded to {}), dry-run enabled; not installing",
                            app.name,
                            downloaded_path.display()
                        );
                    } else {
                        info!(
                            "{}: installing update from {}",
                            app.name,
                            downloaded_path.display()
                        );
                        installer.install(&downloaded_path)?;
                        info!("{}: installation completed", app.name);
                    }
                }
            }
        }
    }

    Ok(())
}