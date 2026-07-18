//! GitHub Releases-based [`UpdateProvider`](crate::UpdateProvider) implementation.

use crate::{UpdateProvider, UpdateType};
use directories::BaseDirs;
use octocrab::models::repos::Release;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{fs::File, path::Path};

const RELEASE_CACHE_TTL_SECS: u64 = 6 * 60 * 60;

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

    /// Fetches the latest release page from the GitHub repository.
    /// This method caches the releases in a local file for 6 hours to reduce API calls.
    pub async fn get_latest_releases(&self) -> Result<Vec<Release>, Box<dyn std::error::Error>> {
        let cache_dir = BaseDirs::new()
            .ok_or("Failed to load base directories")?
            .cache_dir()
            .join("reup")
            .join("github");
        std::fs::create_dir_all(&cache_dir)?;
        let cache_file = cache_dir.join(format!("{}_{}_releases.json", self.owner, self.repo));
        if std::fs::exists(&cache_file).is_ok_and(|x| x) {
            let metadata = std::fs::metadata(&cache_file)?;
            let last_write_time = metadata.modified()?;
            if last_write_time
                .elapsed()
                .is_ok_and(|elapsed| elapsed.as_secs() <= RELEASE_CACHE_TTL_SECS)
            {
                if let Ok(file) = File::open(&cache_file)
                    && let Ok(releases) = serde_json::from_reader::<_, Vec<Release>>(file)
                {
                    return Ok(releases);
                }
                let _ = std::fs::remove_file(&cache_file);
            }
        }
        let octocrab = octocrab::instance();
        let releases = octocrab
            .repos(self.owner.clone(), self.repo.clone())
            .releases()
            .list()
            .send()
            .await?;
        std::fs::write(&cache_file, serde_json::to_vec(&releases.items)?)?;
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
        let releases = self.get_latest_releases().await?;
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
        let releases = self.get_latest_releases().await?;
        for release in releases {
            if release.prerelease && update_type == UpdateType::Stable {
                continue;
            }
            let tag = release.tag_name.to_lowercase();
            let trimmed = tag.trim_start_matches('v');
            let split_idx = trimmed.find(['-', '+']).unwrap_or(trimmed.len());
            let (core, suffix) = trimmed.split_at(split_idx);
            let normalized_core = core
                .split('.')
                .map(|segment| {
                    if segment.chars().all(|c| c.is_ascii_digit()) {
                        segment
                            .parse::<u64>()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|_| segment.to_string())
                    } else {
                        segment.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(".");
            return Ok(Version::parse(&format!("{normalized_core}{suffix}"))?);
        }
        Err("No suitable release found".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use std::time::{SystemTime, UNIX_EPOCH};

    const OWNER: &str = "yt-dlp";
    const REPO: &str = "yt-dlp";
    const RELEASE_TAG: &str = "2026.07.04";
    const ASSET_WINDOWS: &str = "yt-dlp.exe";
    const ASSET_LINUX: &str = "yt-dlp_linux";
    const ASSET_MACOS: &str = "yt-dlp_macos";

    #[cfg(target_os = "windows")]
    const TARGET_ASSET: &str = ASSET_WINDOWS;
    #[cfg(target_os = "linux")]
    const TARGET_ASSET: &str = ASSET_LINUX;
    #[cfg(target_os = "macos")]
    const TARGET_ASSET: &str = ASSET_MACOS;

    fn ensure_rustls_crypto_provider() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    #[test]
    fn builder_requires_owner() {
        let result = GitHubUpdater::builder()
            .repo(REPO)
            .target_asset_name(ASSET_LINUX)
            .build();
        assert_eq!(result, Err(GitHubUpdaterBuilderError::MissingOwner));
    }

    #[test]
    fn builder_requires_repo() {
        let result = GitHubUpdater::builder()
            .owner(OWNER)
            .target_asset_name(ASSET_LINUX)
            .build();
        assert_eq!(result, Err(GitHubUpdaterBuilderError::MissingRepo));
    }

    #[test]
    fn builder_requires_target_asset_name() {
        let result = GitHubUpdater::builder().owner(OWNER).repo(REPO).build();
        assert_eq!(
            result,
            Err(GitHubUpdaterBuilderError::MissingTargetAssetName)
        );
    }

    #[test]
    fn builder_creates_updater_with_all_required_fields() {
        let result = GitHubUpdater::builder()
            .owner(OWNER)
            .repo(REPO)
            .target_asset_name(ASSET_LINUX)
            .build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn yt_dlp_releases_include_expected_tag_and_assets() {
        ensure_rustls_crypto_provider();
        let updater = GitHubUpdater::new(OWNER, REPO, TARGET_ASSET);
        let releases = updater
            .get_latest_releases()
            .await
            .expect("must fetch releases from GitHub");
        let release = releases
            .iter()
            .find(|release| release.tag_name == RELEASE_TAG)
            .expect("expected yt-dlp release tag must exist");
        let asset_names = release
            .assets
            .iter()
            .map(|asset| asset.name.as_str())
            .collect::<Vec<_>>();
        assert!(asset_names.contains(&ASSET_WINDOWS));
        assert!(asset_names.contains(&ASSET_LINUX));
        assert!(asset_names.contains(&ASSET_MACOS));
    }

    #[tokio::test]
    async fn latest_stable_yt_dlp_version_is_parseable() {
        ensure_rustls_crypto_provider();
        let updater = GitHubUpdater::new(OWNER, REPO, TARGET_ASSET);
        let version = updater
            .get_latest_version(UpdateType::Stable)
            .await
            .expect("latest stable release must be parseable");
        assert!(version >= Version::new(2026, 7, 4));
    }

    #[tokio::test]
    async fn download_update_downloads_current_platform_asset() {
        ensure_rustls_crypto_provider();
        let updater = GitHubUpdater::new(OWNER, REPO, TARGET_ASSET);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let destination = std::env::temp_dir().join(format!("reup-test-{unique}-{TARGET_ASSET}"));
        updater
            .download_update(UpdateType::Stable, &destination)
            .await
            .expect("download should succeed");
        let metadata = std::fs::metadata(&destination).expect("downloaded file should exist");
        assert!(metadata.len() > 0, "downloaded file should not be empty");
        let _ = std::fs::remove_file(&destination);
    }
}
