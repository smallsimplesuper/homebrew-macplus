use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::http_client::APP_USER_AGENT;
use crate::utils::AppResult;

const KEYSTONE_BUNDLE_IDS: &[&str] = &[
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "com.google.drivefs",
    "com.google.GoogleUpdater",
];

pub struct KeystoneChecker;

impl KeystoneChecker {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct ChromiumRelease {
    version: String,
}

#[async_trait]
impl UpdateChecker for KeystoneChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::Keystone
    }

    fn can_check(&self, bundle_id: &str, _app_path: &Path, install_source: &AppSource) -> bool {
        *install_source != AppSource::MacAppStore
            && KEYSTONE_BUNDLE_IDS.iter().any(|&id| id == bundle_id)
    }

    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let current = match current_version {
            Some(v) => v,
            None => return Ok(None),
        };

        // For Chrome variants, query the Chromium Dash API
        if bundle_id == "com.google.Chrome" || bundle_id == "com.google.Chrome.canary" {
            let channel = if bundle_id == "com.google.Chrome.canary" {
                "Canary"
            } else {
                "Stable"
            };

            let url = format!(
                "https://chromiumdash.appspot.com/fetch_releases?channel={}&platform=Mac&num=1",
                channel
            );

            let resp = client
                .get(&url)
                .header("User-Agent", APP_USER_AGENT)
                .send()
                .await?;

            if !resp.status().is_success() {
                return Ok(None);
            }

            let releases: Vec<ChromiumRelease> = resp.json().await?;
            if let Some(release) = releases.first() {
                if version_compare::is_newer(current, &release.version) {
                    log::info!(
                        "Keystone: {} has update {} -> {}",
                        bundle_id, current, release.version
                    );
                    return Ok(Some(UpdateInfo {
                        bundle_id: bundle_id.to_string(),
                        current_version: Some(current.to_string()),
                        available_version: release.version.clone(),
                        source_type: UpdateSourceType::Keystone,
                        download_url: None,
                        release_notes_url: Some("https://chromereleases.googleblog.com/".to_string()),
                        release_notes: None,
                        is_paid_upgrade: false,
                        notes: None,
                    }));
                }
            }
            return Ok(None);
        }

        // For other Google apps, fall back to Homebrew cask index
        if let Some(ref index) = context.homebrew_cask_index {
            if let Some(cask_info) = index.lookup(bundle_id, app_path) {
                if version_compare::is_newer(current, &cask_info.version) {
                    log::info!(
                        "Keystone (Homebrew fallback): {} has update {} -> {}",
                        bundle_id, current, cask_info.version
                    );
                    return Ok(Some(UpdateInfo {
                        bundle_id: bundle_id.to_string(),
                        current_version: Some(current.to_string()),
                        available_version: cask_info.version.clone(),
                        source_type: UpdateSourceType::Keystone,
                        download_url: None,
                        release_notes_url: None,
                        release_notes: None,
                        is_paid_upgrade: false,
                        notes: None,
                    }));
                }
            }
        }

        Ok(None)
    }
}
