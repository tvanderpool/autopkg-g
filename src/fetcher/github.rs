use crate::config::{ApplicationConfig, FetcherConfig};
use crate::fetcher::Fetcher;
use crate::types::FetchResult;

use anyhow::{anyhow, Context, Result};
use glob::Pattern;
use log::{info, warn};
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs::File;
use std::io::copy;
use std::path::PathBuf;
use std::time::Duration;

/// GitHub releases API response subset.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// GitHub-based fetcher implementation.
pub struct GitHubFetcher {
    owner: String,
    repo: String,
    file_pattern: Pattern,
    client: Client,
    _app_name: String,
}

impl GitHubFetcher {
    pub fn new(config: &FetcherConfig, app: &ApplicationConfig) -> Result<Self> {
        let repo_str = config
            .repo
            .as_ref()
            .ok_or_else(|| anyhow!("GitHub fetcher requires `repo` field"))?;
        let (owner, repo) = repo_str
            .split_once('/')
            .ok_or_else(|| anyhow!("GitHub repo must be in form `owner/repo`"))?;

        let pattern_str = config.file_pattern.as_deref().unwrap_or("*");

        let file_pattern =
            Pattern::new(pattern_str).with_context(|| format!("Invalid glob pattern: {}", pattern_str))?;

        let client = Client::builder()
            .user_agent("autopkg-rust/0.1")
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            file_pattern,
            client,
            _app_name: app.name.clone(),
        })
    }

    fn latest_release(&self) -> Result<GitHubRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.owner, self.repo
        );
        info!("GitHubFetcher: querying {}", url);

        let resp = self.client.get(&url).send()?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "GitHub API returned non-success status: {}",
                resp.status()
            ));
        }

        let release: GitHubRelease = resp.json()?;
        Ok(release)
    }

    fn download_asset(&self, url: &str, name: &str) -> Result<PathBuf> {
        let mut resp = self.client.get(url).send()?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "Failed to download asset from {}: status {}",
                url,
                resp.status()
            ));
        }

        let tmp_dir = std::env::temp_dir();
        let filename = format!("autopkg-{}-{}", self.repo, name);
        let path = tmp_dir.join(filename);

        let mut out = File::create(&path)?;
        copy(&mut resp, &mut out)?;

        info!("Downloaded asset to {}", path.display());
        Ok(path)
    }

    /// Naive version extraction from a tag like "v1.2.3" or "1.2.3".
    fn normalize_version(tag: &str) -> String {
        let re = Regex::new(r"v?(?P<version>[0-9][0-9A-Za-z\.\-\+]*)").unwrap();
        if let Some(caps) = re.captures(tag) {
            caps["version"].to_string()
        } else {
            tag.to_string()
        }
    }

    /// Very simple semantic version comparison: "1.2.3" style.
    /// Returns true if `remote` is newer than `local`.
    fn is_newer(local: &str, remote: &str) -> bool {
        fn parse(v: &str) -> Vec<u64> {
            v.split('.').filter_map(|s| s.parse::<u64>().ok()).collect()
        }

        let mut local_parts = parse(local);
        let mut remote_parts = parse(remote);

        let max_len = local_parts.len().max(remote_parts.len());
        local_parts.resize(max_len, 0);
        remote_parts.resize(max_len, 0);

        for (l, r) in local_parts.iter().zip(remote_parts.iter()) {
            if r > l {
                return true;
            } else if r < l {
                return false;
            }
        }
        false
    }
}

impl Fetcher for GitHubFetcher {
    fn fetch_if_newer(&self, current_version: &str) -> FetchResult {
        let release = self.latest_release()?;

        let latest_tag = release.tag_name.clone();
        let latest_version = Self::normalize_version(&latest_tag);
        let current_normalized = Self::normalize_version(current_version);

        info!(
            "GitHubFetcher: latest tag={}, normalized={}, current={}",
            latest_tag, latest_version, current_normalized
        );

        if !Self::is_newer(&current_normalized, &latest_version) {
            info!("GitHubFetcher: no newer version available");
            return Ok(None);
        }

        info!(
            "GitHubFetcher: newer version available: {} > {}",
            latest_version, current_normalized
        );

        // Find asset matching the file_pattern
        let asset = release
            .assets
            .into_iter()
            .find(|a| self.file_pattern.matches(&a.name));

        let asset = match asset {
            Some(a) => a,
            None => {
                warn!(
                    "GitHubFetcher: no asset matching pattern '{}' found",
                    self.file_pattern
                );
                return Ok(None);
            }
        };

        info!(
            "GitHubFetcher: selected asset '{}' ({})",
            asset.name, asset.browser_download_url
        );

        let path = self.download_asset(&asset.browser_download_url, &asset.name)?;
        Ok(Some(path))
    }
}