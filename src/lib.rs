#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("reup only supports Windows, macOS, and Linux");

mod github;
mod update;

pub use github::{GitHubUpdater, GitHubUpdaterBuilder, GitHubUpdaterBuilderError};
pub use update::{UpdateProvider, UpdateType};
