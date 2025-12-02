mod config;
mod fetcher;
mod installer;
mod types;

use crate::config::Config;
use crate::fetcher::create_fetcher;
use crate::installer::create_installer;
use crate::types::UpdateCheck;

// Embedded template files
const DEFAULT_CONFIG: &str = include_str!("../config/default_config.yml");
const SYSTEMD_SERVICE: &str = include_str!("../systemd/autopkg.service");
const SYSTEMD_TIMER: &str = include_str!("../systemd/autopkg.timer");

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use log::{error, info, warn};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    /// Install autopkg binary, config, and systemd units
    SelfInstall {
        /// Install directory for the binary (default: /usr/local/bin)
        #[arg(long, value_name = "PATH", default_value = "/usr/local/bin")]
        install_dir: PathBuf,

        /// Config file path (default: /etc/autopkg/config.yml)
        #[arg(long, value_name = "PATH", default_value = "/etc/autopkg/config.yml")]
        config_path: PathBuf,
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
        Commands::SelfInstall {
            install_dir,
            config_path,
        } => self_install_command(install_dir, config_path),
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

fn save_config(config: &Config, config_path: &Path) -> Result<()> {
    let config_yaml = serde_yaml::to_string(config)
        .with_context(|| "Failed to serialize config to YAML")?;
    fs::write(config_path, config_yaml)
        .with_context(|| format!("Failed to write config file to {}", config_path.display()))?;
    info!("Config file updated: {}", config_path.display());
    Ok(())
}

fn run_command(config: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let (mut config, config_path) = load_config(config)?;
    info!(
        "Loaded {} application(s) from config",
        config.applications.len()
    );

    let mut config_updated = false;

    for app in &mut config.applications {
        info!("Processing application: {}", app.name);

        if let Err(e) = process_application(app, dry_run, &mut config_updated) {
            error!(
                "Application '{}' failed: {:?}. Continuing with others.",
                app.name, e
            );
        }
    }

    // Save config if any application updated it
    if config_updated {
        save_config(&config, &config_path)?;
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

fn process_application(
    app: &mut config::ApplicationConfig,
    dry_run: bool,
    config_updated: &mut bool,
) -> Result<()> {
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

                        // Update package_name in config if it was not set and this is a deb installer
                        if app.installer.r#type == "deb" && app.package_name.is_none() {
                            info!(
                                "{}: updating config to set package_name = {}",
                                app.name, app.name
                            );
                            app.package_name = Some(app.name.clone());
                            *config_updated = true;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
fn self_install_command(install_dir: PathBuf, config_path: PathBuf) -> Result<()> {
    info!("Starting self-install process");

    // 1. Install binary
    install_binary(&install_dir)?;

    // 2. Install config file
    install_config_file(&config_path)?;

    // 3. Install systemd units
    install_systemd_units()?;

    // 4. Reload systemd and enable timer
    enable_systemd_timer()?;

    info!("Self-install completed successfully!");
    info!("Binary installed to: {}/autopkg", install_dir.display());
    info!("Config file at: {}", config_path.display());
    info!("systemd units installed and timer enabled");
    info!("");
    info!("You can now:");
    info!("  - Edit the config file: {}", config_path.display());
    info!("  - Check timer status: systemctl status autopkg.timer");
    info!(
        "  - Run manually: autopkg run --config {}",
        config_path.display()
    );

    Ok(())
}

fn install_binary(install_dir: &Path) -> Result<()> {
    info!("Installing binary to {}/autopkg", install_dir.display());

    // Get the path of the currently running executable
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;

    // Create the install directory if it doesn't exist
    if !install_dir.exists() {
        fs::create_dir_all(install_dir).with_context(|| {
            format!(
                "Failed to create install directory: {}",
                install_dir.display()
            )
        })?;
    }

    // Target path is always "autopkg" regardless of source name
    let target_path = install_dir.join("autopkg");

    // Check if target already exists
    if target_path.exists() {
        info!(
            "Binary already exists at {}, skipping copy",
            target_path.display()
        );
        return Ok(());
    }

    // Copy the binary
    fs::copy(&current_exe, &target_path).with_context(|| {
        format!(
            "Failed to copy binary to {}. Do you have permission to write to this directory?",
            target_path.display()
        )
    })?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_path, perms).with_context(|| {
            format!(
                "Failed to set executable permissions on {}",
                target_path.display()
            )
        })?;
    }

    info!("Binary installed successfully to {}", target_path.display());
    Ok(())
}

fn install_config_file(config_path: &Path) -> Result<()> {
    info!("Installing config file to {}", config_path.display());

    // Check if config already exists
    if config_path.exists() {
        info!(
            "Config file already exists at {}, leaving it unchanged",
            config_path.display()
        );
        return Ok(());
    }

    // Create parent directories if they don't exist
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create config directory: {}. Do you have permission?",
                    parent.display()
                )
            })?;
        }
    }

    // Write the default config
    fs::write(config_path, DEFAULT_CONFIG).with_context(|| {
        format!(
            "Failed to write config file to {}. Do you have permission?",
            config_path.display()
        )
    })?;

    info!(
        "Config file created successfully at {}",
        config_path.display()
    );
    Ok(())
}

fn install_systemd_units() -> Result<()> {
    let systemd_dir = Path::new("/etc/systemd/system");

    info!("Installing systemd units to {}", systemd_dir.display());

    // Check if systemd directory exists
    if !systemd_dir.exists() {
        return Err(anyhow!(
            "systemd directory {} does not exist. Is this a systemd-based system?",
            systemd_dir.display()
        ));
    }

    // Install service file
    let service_path = systemd_dir.join("autopkg.service");
    if service_path.exists() {
        info!(
            "Service file already exists at {}, skipping",
            service_path.display()
        );
    } else {
        fs::write(&service_path, SYSTEMD_SERVICE)
            .with_context(|| format!(
                "Failed to write service file to {}. Do you have permission? (Try running with sudo)",
                service_path.display()
            ))?;
        info!(
            "Service file created successfully at {}",
            service_path.display()
        );
    }

    // Install timer file
    let timer_path = systemd_dir.join("autopkg.timer");
    if timer_path.exists() {
        info!(
            "Timer file already exists at {}, skipping",
            timer_path.display()
        );
    } else {
        fs::write(&timer_path, SYSTEMD_TIMER).with_context(|| {
            format!(
                "Failed to write timer file to {}. Do you have permission? (Try running with sudo)",
                timer_path.display()
            )
        })?;
        info!(
            "Timer file created successfully at {}",
            timer_path.display()
        );
    }

    Ok(())
}

fn enable_systemd_timer() -> Result<()> {
    info!("Reloading systemd daemon");

    // Check if systemctl is available
    if which::which("systemctl").is_err() {
        return Err(anyhow!(
            "systemctl command not found. Is this a systemd-based system?"
        ));
    }

    // Reload systemd daemon
    let reload_output = Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .context("Failed to execute 'systemctl daemon-reload'")?;

    if !reload_output.status.success() {
        let stderr = String::from_utf8_lossy(&reload_output.stderr);
        return Err(anyhow!("Failed to reload systemd daemon: {}", stderr));
    }

    info!("Systemd daemon reloaded successfully");

    // Enable and start the timer
    info!("Enabling and starting autopkg.timer");
    let enable_output = Command::new("systemctl")
        .arg("enable")
        .arg("--now")
        .arg("autopkg.timer")
        .output()
        .context("Failed to execute 'systemctl enable --now autopkg.timer'")?;

    if !enable_output.status.success() {
        let stderr = String::from_utf8_lossy(&enable_output.stderr);
        return Err(anyhow!(
            "Failed to enable and start autopkg.timer: {}. Do you have permission? (Try running with sudo)",
            stderr
        ));
    }

    info!("autopkg.timer enabled and started successfully");
    Ok(())
}
