use std::path::PathBuf;

/// Whether and how to check for updates.
#[derive(Debug, Clone)]
pub enum UpdateCheck {
    /// Don't check (e.g., pinned version)
    No,
    /// Check for updates, with the current installed version
    Yes(String),
}

/// Common result type for components.
pub type FetchResult = anyhow::Result<Option<PathBuf>>;
