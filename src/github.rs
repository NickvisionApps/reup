//! GitHub Releases-based [`UpdateProvider`](crate::UpdateProvider) implementation.

use crate::{UpdateProvider, UpdateType};
use octocrab::models::repos::Release;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{fs::File, path::Path};

/// Downloads update artifacts and versions from a GitHub repository's releases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubUpdater {
    owner: String,
    repo: String,
    target_asset_name: String,
}

/// Builder for constructing a [`GitHubUpdater`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitHubUpdaterBuilder {
    owner: Option<String>,
    repo: Option<String>,
    target_asset_name: Option<String>,
}

/// Errors returned when required [`GitHubUpdaterBuilder`] fields are missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubUpdaterBuilderError {
    /// The repository owner was not provided.
    MissingOwner,
    /// The repository name was not provided.
    MissingRepo,
    /// The expected release asset filename was not provided.
    MissingTargetAssetName,
}

impl GitHubUpdater {
    /// Creates a new updater for a repository and asset filename.
    pub fn new<O: Into<String>, R: Into<String>, T: Into<String>>(
        owner: O,
        repo: R,
        target_asset_name: T,
    ) -> Self {
        Self {
            owner: owner.into(),
            repo: repo.into(),
            target_asset_name: target_asset_name.into(),
        }
    }

    /// Returns a builder for constructing a [`GitHubUpdater`].
    pub fn builder() -> GitHubUpdaterBuilder {
        GitHubUpdaterBuilder::default()
    }

    /// Fetches all releases from the GitHub repository.
    pub async fn get_all_releases(&self) -> Result<Vec<Release>, Box<dyn std::error::Error>> {
        let octocrab = octocrab::instance();
        let releases = octocrab
            .repos(self.owner.clone(), self.repo.clone())
            .releases()
            .list()
            .send()
            .await?;
        Ok(releases.items)
    }
}

impl GitHubUpdaterBuilder {
    /// Sets the repository owner (user or organization).
    pub fn owner<O: Into<String>>(mut self, owner: O) -> Self {
        self.owner = Some(owner.into());
        self
    }

    /// Sets the repository name.
    pub fn repo<R: Into<String>>(mut self, repo: R) -> Self {
        self.repo = Some(repo.into());
        self
    }

    /// Sets the expected release asset filename to download.
    pub fn target_asset_name<T: Into<String>>(mut self, target_asset_name: T) -> Self {
        self.target_asset_name = Some(target_asset_name.into());
        self
    }

    /// Builds a [`GitHubUpdater`] if all required fields are set.
    pub fn build(self) -> Result<GitHubUpdater, GitHubUpdaterBuilderError> {
        let owner = self.owner.ok_or(GitHubUpdaterBuilderError::MissingOwner)?;
        let repo = self.repo.ok_or(GitHubUpdaterBuilderError::MissingRepo)?;
        let target_asset_name = self
            .target_asset_name
            .ok_or(GitHubUpdaterBuilderError::MissingTargetAssetName)?;

        Ok(GitHubUpdater {
            owner,
            repo,
            target_asset_name,
        })
    }
}

impl UpdateProvider for GitHubUpdater {
    async fn download_update(
        &self,
        update_type: UpdateType,
        destination: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let releases = self.get_all_releases().await?;
        for release in releases {
            if release.prerelease && update_type == UpdateType::Stable {
                continue;
            }
            for asset in release.assets {
                if asset.name != self.target_asset_name {
                    continue;
                }
                let mut file = File::create(destination)?;
                let expected_hash = asset
                    .digest
                    .ok_or("No digest found for asset")?
                    .to_lowercase()
                    .trim_start_matches("sha256:")
                    .to_string();
                let bytes = reqwest::get(asset.browser_download_url.as_str())
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?
                    .as_ref()
                    .to_owned();
                std::io::copy(&mut &bytes[..], &mut file)?;
                let real_hash = hex::encode(Sha256::digest(&bytes));
                if real_hash != expected_hash {
                    return Err(format!(
                        "Hash mismatch: expected {}, got {}",
                        expected_hash, real_hash
                    )
                    .into());
                }
                return Ok(());
            }
        }
        Err("No suitable release found".into())
    }

    async fn get_latest_version(
        &self,
        update_type: UpdateType,
    ) -> Result<Version, Box<dyn std::error::Error>> {
        let octocrab = octocrab::instance();
        let releases = octocrab
            .repos(self.owner.clone(), self.repo.clone())
            .releases()
            .list()
            .send()
            .await?;
        for release in releases.items {
            if release.prerelease && update_type == UpdateType::Stable {
                continue;
            }
            return Ok(Version::parse(
                release.tag_name.to_lowercase().trim_start_matches('v'),
            )?);
        }
        Err("No suitable release found".into())
    }
}
