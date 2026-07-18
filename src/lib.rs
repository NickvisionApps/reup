//! Cross-platform application update helpers with a GitHub Releases provider.

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("reup only supports Windows, macOS, and Linux");

/// GitHub Releases-backed update provider implementation.
mod github;
/// Core update abstractions shared by update providers.
mod update;

pub use github::{GitHubUpdater, GitHubUpdaterBuilder, GitHubUpdaterBuilderError};
pub use update::{UpdateProvider, UpdateType};
