//! Cross-platform application update helpers for desktop Rust applications.
//!
//! The [`GitHubUpdater`] provider discovers releases from a GitHub repository,
//! resolves the latest version for a release channel, and downloads a matching
//! release asset with SHA-256 verification.
//!
//! # Examples
//!
//! ```no_run
//! use reup::{GitHubUpdater, UpdateProvider, UpdateType};
//! use std::path::Path;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let updater = GitHubUpdater::builder()
//!     .owner("example")
//!     .repo("my-app")
//!     .target_asset_name("my-app-linux")
//!     .build()?;
//!
//! let latest = updater.get_latest_version(UpdateType::Stable).await?;
//! println!("latest stable version: {latest}");
//!
//! // Downloading is optional; the destination is created or replaced.
//! updater
//!     .download_update(UpdateType::Stable, Path::new("my-app.new"), |downloaded, total| {
//!         println!("{downloaded}/{total} bytes downloaded");
//!     })
//!     .await?;
//! # Ok(())
//! # }
//! ```

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("reup only supports Windows, macOS, and Linux");

/// GitHub Releases-backed update provider implementation.
///
/// Use [`GitHubUpdater::builder`] when constructing an updater with named
/// fields, or [`GitHubUpdater::new`] for a positional constructor.
mod github;
/// Core update abstractions shared by update providers.
mod update;

pub use github::{GitHubUpdater, GitHubUpdaterBuilder, GitHubUpdaterBuilderError};
pub use update::{UpdateProvider, UpdateType};
