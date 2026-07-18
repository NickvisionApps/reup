use crate::{UpdateProvider, UpdateType};
use semver::Version;
use sha2::{Digest, Sha256};
use std::{fs::File, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubUpdater {
    owner: String,
    repo: String,
    target_asset_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitHubUpdaterBuilder {
    owner: Option<String>,
    repo: Option<String>,
    target_asset_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubUpdaterBuilderError {
    MissingOwner,
    MissingRepo,
    MissingTargetAssetName,
}

impl GitHubUpdater {
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

    pub fn builder() -> GitHubUpdaterBuilder {
        GitHubUpdaterBuilder::default()
    }
}

impl GitHubUpdaterBuilder {
    pub fn owner<O: Into<String>>(mut self, owner: O) -> Self {
        self.owner = Some(owner.into());
        self
    }

    pub fn repo<R: Into<String>>(mut self, repo: R) -> Self {
        self.repo = Some(repo.into());
        self
    }

    pub fn target_asset_name<T: Into<String>>(mut self, target_asset_name: T) -> Self {
        self.target_asset_name = Some(target_asset_name.into());
        self
    }

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
