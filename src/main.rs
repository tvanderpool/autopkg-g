mod config;
mod fetcher;
mod installer;
mod types;

use crate::config::Config;
use crate::fetcher::create_fetcher;
use crate::installer::create_installer;
use crate::types::UpdateCheck;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use log::{error, info, warn};
use std::fs;
use std::path::PathBuf;

/// Auto-updater tool for applications defined in a YAML config.
#[derive(Parser, Debug)]
#[command(name = "autopkg")]
#[command(author, version, about)]
struct Cli {
    /// Log level (error, warn, info, debug, trace)
    #[arg(long, value_name = "LEVEL", default_value = "info", global = true)]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run update checks (and installs, unless --dry-run)
    Run {
        /// Path to config file (default: autopkg.yml in current directory)
        #[arg(long, value_name = "PATH")]
        config: Option<PathBuf>,

        /// Check for updates without installing
        #[arg(long)]
        dry_run: bool,
    },

    /// Show the parsed configuration
    ShowConfig {
        /// Path to config file (default: autopkg.yml in current directory)
        #[arg(long, value_name = "PATH")]
        config: Option<PathBuf>,
    },
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

    match cli.command {
        Commands::Run { config, dry_run } => run_command(config, dry_run),
        Commands::ShowConfig { config } => show_config_command(config),
    }
}

fn load_config(config: Option<PathBuf>) -> Result<(Config, PathBuf)> {
    let config_path = config.unwrap_or_else(|| PathBuf::from("autopkg.yml"));
    info!("Using config file: {}", config_path.display());

    let config_contents =
        fs::read_to_string(&config_path).with_context(|| "Failed to read config file")?;
    let config: Config =
        serde_yaml::from_str(&config_contents).with_context(|| "Failed to parse config YAML")?;
    Ok((config, config_path))
}

fn run_command(config: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let (config, _) = load_config(config)?;
    info!(
        "Loaded {} application(s) from config",
        config.applications.len()
    );

    for app in &config.applications {
        info!("Processing application: {}", app.name);

        if let Err(e) = process_application(app, dry_run) {
            error!(
                "Application '{}' failed: {:?}. Continuing with others.",
                app.name, e
            );
        }
    }

    Ok(())
}

fn show_config_command(config: Option<PathBuf>) -> Result<()> {
    let (config, config_path) = load_config(config)?;
    info!(
        "Configuration from {} successfully parsed:",
        config_path.display()
    );

    // Pretty-print the config to stdout (still logging to stderr)
    println!(
        "{}",
        serde_yaml::to_string(&config).unwrap_or_else(|_| "<failed to serialize config>".into())
    );
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