//! GitHub Releases-based [`UpdateProvider`](crate::UpdateProvider) implementation.
//!
//! Releases are read from the GitHub API and cached for six hours in the
//! platform cache directory. Downloaded assets are verified against the
//! SHA-256 digest reported by GitHub before the operation succeeds.

use crate::{UpdateProvider, UpdateType};
use directories::BaseDirs;
use octocrab::models::repos::Release;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{
    fmt::{Display, Formatter},
    fs::File,
    io::{Read, Write},
    path::Path,
};

const RELEASE_CACHE_TTL_SECS: u64 = 6 * 60 * 60;

/// Downloads update artifacts and versions from a GitHub repository's releases.
///
/// `target_asset_name` must exactly match the name of the release asset to
/// download. The provider uses the first suitable release returned by GitHub.
///
/// # Examples
///
/// ```
/// use reup::GitHubUpdater;
///
/// let updater = GitHubUpdater::new("example", "my-app", "my-app-linux");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubUpdater {
    owner: String,
    repo: String,
    target_asset_name: String,
}

/// Builder for constructing a [`GitHubUpdater`].
///
/// # Examples
///
/// ```
/// use reup::GitHubUpdater;
///
/// let updater = GitHubUpdater::builder()
///     .owner("example")
///     .repo("my-app")
///     .target_asset_name("my-app-linux")
///     .build()
///     .expect("all updater fields are set");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitHubUpdaterBuilder {
    owner: Option<String>,
    repo: Option<String>,
    target_asset_name: Option<String>,
}

/// Errors returned when required [`GitHubUpdaterBuilder`] fields are missing.
///
/// # Examples
///
/// ```
/// use reup::{GitHubUpdater, GitHubUpdaterBuilderError};
///
/// let error = GitHubUpdater::builder()
///     .repo("my-app")
///     .target_asset_name("my-app-linux")
///     .build()
///     .expect_err("the owner is required");
///
/// assert_eq!(error, GitHubUpdaterBuilderError::MissingOwner);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubUpdaterBuilderError {
    /// The GitHub repository owner or organization was not provided.
    MissingOwner,
    /// The GitHub repository name was not provided.
    MissingRepo,
    /// The release asset filename to download was not provided.
    MissingTargetAssetName,
}

impl GitHubUpdater {
    /// Creates an updater for a repository and release asset filename.
    ///
    /// `owner` and `repo` identify the GitHub repository. `target_asset_name`
    /// must match the release asset that should be downloaded, including its
    /// platform-specific suffix when applicable.
    ///
    /// This constructor only stores configuration; it does not contact GitHub.
    ///
    /// # Examples
    ///
    /// ```
    /// use reup::GitHubUpdater;
    ///
    /// let updater = GitHubUpdater::new("example", "my-app", "my-app.exe");
    /// ```
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

    /// Returns an empty builder for constructing a [`GitHubUpdater`].
    ///
    /// # Examples
    ///
    /// ```
    /// use reup::GitHubUpdater;
    ///
    /// let updater = GitHubUpdater::builder()
    ///     .owner("example")
    ///     .repo("my-app")
    ///     .target_asset_name("my-app-linux")
    ///     .build()
    ///     .expect("all updater fields are set");
    /// ```
    pub fn builder() -> GitHubUpdaterBuilder {
        GitHubUpdaterBuilder::default()
    }

    /// Returns the configured repository owner or organization.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Returns the configured repository name.
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// Returns the configured release asset filename to download.
    pub fn target_asset_name(&self) -> &str {
        &self.target_asset_name
    }

    fn get_latest_releases(&self) -> Result<Vec<Release>, Box<dyn std::error::Error>> {
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
        let client = reqwest::blocking::Client::builder()
            .user_agent(format!("{}-updater", self.repo))
            .build()?;
        let releases = client
            .get(format!(
                "https://api.github.com/repos/{}/{}/releases",
                self.owner, self.repo
            ))
            .send()?
            .error_for_status()?
            .json::<Vec<Release>>()?;
        std::fs::write(&cache_file, serde_json::to_vec(&releases)?)?;
        Ok(releases)
    }
}

impl GitHubUpdaterBuilder {
    /// Sets the repository owner or organization.
    ///
    /// This field is required before calling [`Self::build`].
    pub fn owner<O: Into<String>>(mut self, owner: O) -> Self {
        self.owner = Some(owner.into());
        self
    }

    /// Sets the repository name.
    ///
    /// This field is required before calling [`Self::build`].
    pub fn repo<R: Into<String>>(mut self, repo: R) -> Self {
        self.repo = Some(repo.into());
        self
    }

    /// Sets the exact release asset filename to download.
    ///
    /// This field is required before calling [`Self::build`]. The name must
    /// match the GitHub release asset exactly.
    pub fn target_asset_name<T: Into<String>>(mut self, target_asset_name: T) -> Self {
        self.target_asset_name = Some(target_asset_name.into());
        self
    }

    /// Builds a [`GitHubUpdater`] if all required fields are set.
    ///
    /// # Errors
    ///
    /// Returns the first missing-field error in owner, repository, and asset
    /// name order.
    ///
    /// # Examples
    ///
    /// ```
    /// use reup::GitHubUpdater;
    ///
    /// let updater = GitHubUpdater::builder()
    ///     .owner("example")
    ///     .repo("my-app")
    ///     .target_asset_name("my-app-linux")
    ///     .build()?;
    /// # Ok::<(), reup::GitHubUpdaterBuilderError>(())
    /// ```
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
    /// Downloads the first matching release asset and verifies its SHA-256
    /// digest against the digest reported by GitHub.
    ///
    /// Stable updates skip prerelease releases. Preview updates may use either
    /// stable or prerelease releases. The file at `destination` is created or
    /// replaced before the digest is checked. `on_progress` is called as bytes
    /// arrive with `(bytes_downloaded, total_bytes)`; `total_bytes` is `0` if
    /// the server does not report a content length.
    ///
    /// # Errors
    ///
    /// Returns an error if release metadata cannot be fetched, no suitable
    /// release contains `target_asset_name`, the asset has no SHA-256 digest,
    /// the download fails, the destination cannot be written, or the computed
    /// digest does not match GitHub's digest.
    fn download_update(
        &self,
        update_type: UpdateType,
        destination: &Path,
        on_progress: impl Fn(u64, u64),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let releases = self.get_latest_releases()?;
        for release in releases {
            if release.prerelease && update_type == UpdateType::Stable {
                continue;
            }
            for asset in release.assets {
                if asset.name != self.target_asset_name {
                    continue;
                }
                let expected_hash = asset
                    .digest
                    .ok_or("No digest found for asset")?
                    .to_lowercase()
                    .trim_start_matches("sha256:")
                    .to_string();
                let mut response = reqwest::blocking::get(asset.browser_download_url.as_str())?
                    .error_for_status()?;
                let total = response.content_length().unwrap_or(0);
                let mut file = File::create(destination)?;
                let mut hasher = Sha256::new();
                let mut downloaded = 0u64;
                let mut buffer = [0u8; 8192];
                loop {
                    let n = response.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buffer[..n])?;
                    hasher.update(&buffer[..n]);
                    downloaded += n as u64;
                    on_progress(downloaded, total);
                }
                let real_hash = hex::encode(hasher.finalize());
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

    /// Returns the semantic version parsed from the first suitable GitHub
    /// release tag.
    ///
    /// A leading `v` is ignored, and numeric version components are normalized
    /// so tags such as `2026.07.04` can be parsed by `semver`.
    ///
    /// # Errors
    ///
    /// Returns an error if release metadata cannot be fetched, no suitable
    /// release exists, or the selected release tag is not a valid semantic
    /// version.
    fn get_latest_version(
        &self,
        update_type: UpdateType,
    ) -> Result<Version, Box<dyn std::error::Error>> {
        let releases = self.get_latest_releases()?;
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

impl Display for GitHubUpdaterBuilderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MissingOwner => "GitHub repository owner was not provided",
            Self::MissingRepo => "GitHub repository name was not provided",
            Self::MissingTargetAssetName => "release asset filename was not provided",
        })
    }
}

impl std::error::Error for GitHubUpdaterBuilderError {}

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

    #[test]
    fn yt_dlp_releases_include_expected_tag_and_assets() {
        ensure_rustls_crypto_provider();
        let updater = GitHubUpdater::new(OWNER, REPO, TARGET_ASSET);
        let releases = updater
            .get_latest_releases()
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

    #[test]
    fn latest_stable_yt_dlp_version_is_parseable() {
        ensure_rustls_crypto_provider();
        let updater = GitHubUpdater::new(OWNER, REPO, TARGET_ASSET);
        let version = updater
            .get_latest_version(UpdateType::Stable)
            .expect("latest stable release must be parseable");
        assert!(version >= Version::new(2026, 7, 4));
    }

    #[test]
    fn download_update_downloads_current_platform_asset() {
        ensure_rustls_crypto_provider();
        let updater = GitHubUpdater::new(OWNER, REPO, TARGET_ASSET);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let destination = std::env::temp_dir().join(format!("reup-test-{unique}-{TARGET_ASSET}"));
        let last_downloaded = std::sync::atomic::AtomicU64::new(0);
        updater
            .download_update(UpdateType::Stable, &destination, |downloaded, _total| {
                last_downloaded.store(downloaded, std::sync::atomic::Ordering::Relaxed);
            })
            .expect("download should succeed");
        assert!(
            last_downloaded.load(std::sync::atomic::Ordering::Relaxed) > 0,
            "progress callback should have fired"
        );
        let metadata = std::fs::metadata(&destination).expect("downloaded file should exist");
        assert!(metadata.len() > 0, "downloaded file should not be empty");
        let _ = std::fs::remove_file(&destination);
    }
}
