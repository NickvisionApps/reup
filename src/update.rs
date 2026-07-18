//! Core update traits and type definitions used by provider implementations.

use semver::Version;
use std::path::Path;

/// Selects which release channel should be queried for updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateType {
    /// Only stable, non-prerelease versions.
    Stable,
    /// Includes prerelease/preview versions.
    Preview,
}

/// Defines the behavior required for downloading and resolving application updates.
pub trait UpdateProvider {
    /// Downloads an update artifact for the selected release channel to `destination`.
    fn download_update(
        &self,
        update_type: UpdateType,
        destination: &Path,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
    /// Returns the latest available semantic version for the selected release channel.
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
        ) -> Result<(), Box<dyn std::error::Error>> {
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
            .download_update(UpdateType::Stable, &path)
            .await
            .expect("mock download_update should succeed");
        let version = provider
            .get_latest_version(UpdateType::Preview)
            .await
            .expect("mock get_latest_version should succeed");
        assert_eq!(version, Version::new(1, 2, 3));
    }
}
