pub mod github;

use crate::config::{ApplicationConfig, FetcherConfig};
use crate::types::FetchResult;
use anyhow::{anyhow, Result};

/// Trait for fetching updates from a source.
pub trait Fetcher {
    /// If a newer version than `current_version` is available, downloads it and
    /// returns the local path. Otherwise, returns `Ok(None)`.
    fn fetch_if_newer(&self, current_version: &str) -> FetchResult;
}

/// Factory for fetchers.
pub fn create_fetcher(config: &FetcherConfig, app: &ApplicationConfig) -> Result<Box<dyn Fetcher>> {
    match config.r#type.as_str() {
        "github" => Ok(Box::new(github::GitHubFetcher::new(config, app)?)),
        other => Err(anyhow!("Unknown fetcher type: {}", other)),
    }
}
