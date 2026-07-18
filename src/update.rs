use semver::Version;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateType {
    Stable,
    Preview,
}

pub trait UpdateProvider {
    fn download_update(
        &self,
        update_type: UpdateType,
        destination: &Path,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
    fn get_latest_version(
        &self,
        update_type: UpdateType,
    ) -> impl std::future::Future<Output = Result<Version, Box<dyn std::error::Error>>> + Send;
}
