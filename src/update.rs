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
