//! Core update traits and type definitions used by provider implementations.
//!
//! Implement [`UpdateProvider`] to connect an update source to the common
//! version lookup and artifact download API.

use semver::Version;
use std::ops::ControlFlow;
use std::path::Path;

/// Selects which release channel should be queried for updates.
///
/// A provider should always include stable releases. [`Preview`] additionally
/// permits prerelease releases.
///
/// [`Preview`]: Self::Preview
///
/// # Examples
///
/// ```
/// use reup::UpdateType;
///
/// let stable = UpdateType::Stable;
/// let preview = UpdateType::Preview;
/// assert_ne!(stable, preview);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateType {
    /// Selects stable, non-prerelease releases.
    Stable,
    /// Selects stable and prerelease/preview releases.
    Preview,
}

/// Defines the behavior required for downloading and resolving application updates.
///
/// Implementors choose how releases are discovered, how release tags are
/// parsed, and how artifacts are verified. Methods return boxed errors so
/// providers can expose errors from their network, filesystem, or parsing
/// layers without imposing a provider-specific error type on callers.
///
/// # Examples
///
/// ```
/// use reup::{UpdateProvider, UpdateType};
/// use semver::Version;
/// use std::path::Path;
///
/// struct LocalProvider;
///
/// impl UpdateProvider for LocalProvider {
///     async fn download_update(
///         &self,
///         _update_type: UpdateType,
///         destination: &Path,
///         on_progress: impl Fn(u64, u64) -> std::ops::ControlFlow<()> + Send,
///     ) -> Result<(), Box<dyn std::error::Error>> {
///         std::fs::write(destination, b"update")?;
///         if on_progress(4, 4).is_break() {
///             return Err("cancelled".into());
///         }
///         Ok(())
///     }
///
///     async fn get_latest_version(
///         &self,
///         _update_type: UpdateType,
///     ) -> Result<Version, Box<dyn std::error::Error>> {
///         Ok(Version::new(1, 2, 3))
///     }
/// }
/// ```
pub trait UpdateProvider {
    /// Downloads an update artifact for the selected release channel.
    ///
    /// The provider creates or replaces the file at `destination`. `on_progress`
    /// is called as bytes arrive with `(bytes_downloaded, total_bytes)`;
    /// `total_bytes` is `0` when the size is unknown. Returning
    /// [`ControlFlow::Break`] from `on_progress` cancels the download; the
    /// provider stops, discards the partial artifact, and returns an error. A
    /// provider should also return an error when no suitable release or matching
    /// artifact is available, or when downloading or verifying the artifact fails.
    ///
    /// # Errors
    ///
    /// Returns an error when the download is cancelled, when the provider cannot
    /// resolve or download a suitable artifact, or cannot write it to
    /// `destination`.
    fn download_update(
        &self,
        update_type: UpdateType,
        destination: &Path,
        on_progress: impl Fn(u64, u64) -> ControlFlow<()> + Send,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;

    /// Returns the latest available semantic version for the selected channel.
    ///
    /// The returned version is parsed from the provider's release metadata.
    /// Providers should return an error when no suitable release exists or its
    /// version cannot be parsed as a [`semver::Version`].
    ///
    /// # Errors
    ///
    /// Returns an error when releases cannot be queried, no release matches
    /// `update_type`, or the selected release has an invalid version.
    fn get_latest_version(
        &self,
        update_type: UpdateType,
    ) -> impl std::future::Future<Output = Result<Version, Box<dyn std::error::Error>>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct MockProvider;

    impl UpdateProvider for MockProvider {
        async fn download_update(
            &self,
            _update_type: UpdateType,
            _destination: &Path,
            on_progress: impl Fn(u64, u64) -> ControlFlow<()> + Send,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if on_progress(1, 1).is_break() {
                return Err("Download cancelled".into());
            }
            Ok(())
        }

        async fn get_latest_version(
            &self,
            _update_type: UpdateType,
        ) -> Result<Version, Box<dyn std::error::Error>> {
            Ok(Version::new(1, 2, 3))
        }
    }

    #[test]
    fn update_type_variants_are_distinct() {
        assert_ne!(UpdateType::Stable, UpdateType::Preview);
    }

    #[tokio::test]
    async fn update_provider_contract_can_be_implemented() {
        let provider = MockProvider;
        let path = std::env::temp_dir().join("reup-update-provider-contract");
        provider
            .download_update(UpdateType::Stable, &path, |_, _| ControlFlow::Continue(()))
            .await
            .expect("mock download_update should succeed");
        let version = provider
            .get_latest_version(UpdateType::Preview)
            .await
            .expect("mock get_latest_version should succeed");
        assert_eq!(version, Version::new(1, 2, 3));
    }
}
