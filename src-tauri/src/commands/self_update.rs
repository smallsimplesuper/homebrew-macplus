use serde::Serialize;
use tauri::State;

use crate::updaters::github_releases::check_github_release;
use crate::updaters::version_compare;
use crate::utils::brew::brew_path;
use crate::utils::AppError;

const SELF_REPO_OWNER: &str = "smallsimplesuper";
const SELF_REPO_NAME: &str = "homebrew-macplus";
const SELF_BUNDLE_ID: &str = "com.macplus.app";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfUpdateInfo {
    pub available_version: String,
    pub current_version: String,
    pub release_notes_url: Option<String>,
    pub download_url: Option<String>,
    pub can_brew_upgrade: bool,
}

/// Standalone check that can be called from both the Tauri command and the scheduler.
pub async fn check_self_update_inner(client: &reqwest::Client) -> Option<SelfUpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION");

    let update = check_github_release(
        SELF_REPO_OWNER,
        SELF_REPO_NAME,
        SELF_BUNDLE_ID,
        Some(current_version),
        client,
    )
    .await
    .ok()
    .flatten()?;

    // Double-check: the version from GitHub must actually be newer
    if !version_compare::is_newer(current_version, &update.available_version) {
        return None;
    }

    // Check if macPlus is installed via Homebrew cask
    let can_brew_upgrade = check_brew_installed();

    Some(SelfUpdateInfo {
        available_version: update.available_version,
        current_version: current_version.to_string(),
        release_notes_url: update.release_notes_url,
        download_url: update.download_url,
        can_brew_upgrade,
    })
}

/// Check whether macPlus is installed as a Homebrew cask.
fn check_brew_installed() -> bool {
    let Some(brew) = brew_path() else {
        return false;
    };

    std::process::Command::new(brew.as_os_str())
        .current_dir("/tmp")
        .args(["list", "--cask", "macplus"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn check_self_update(
    http_client: State<'_, reqwest::Client>,
) -> Result<Option<SelfUpdateInfo>, AppError> {
    Ok(check_self_update_inner(http_client.inner()).await)
}
