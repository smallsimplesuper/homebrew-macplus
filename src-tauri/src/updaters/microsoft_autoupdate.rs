use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::cask_sha_checker::{self, CaskShaResult};
use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

/// Maps bundle IDs to the XML element names used in macadmins.software/latest.xml
fn microsoft_apps() -> &'static HashMap<&'static str, &'static str> {
    static APPS: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    APPS.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("com.microsoft.Word", "word");
        m.insert("com.microsoft.Excel", "excel");
        m.insert("com.microsoft.Powerpoint", "powerpoint");
        m.insert("com.microsoft.Outlook", "outlook");
        m.insert("com.microsoft.onenote.mac", "onenote");
        m.insert("com.microsoft.teams2", "teams");
        m.insert("com.microsoft.teams", "teams");
        m.insert("com.microsoft.OneDrive", "onedrive");
        m.insert("com.microsoft.edgemac", "edge");
        m.insert("com.microsoft.VSCode", "vscode");
        m
    })
}

/// Hardcoded bundle_id → cask_token mapping for common Microsoft apps.
pub const MICROSOFT_CASK_TOKENS: &[(&str, &str)] = &[
    ("com.microsoft.Word", "microsoft-word"),
    ("com.microsoft.Excel", "microsoft-excel"),
    ("com.microsoft.Powerpoint", "microsoft-powerpoint"),
    ("com.microsoft.Outlook", "microsoft-outlook"),
    ("com.microsoft.onenote.mac", "microsoft-onenote"),
    ("com.microsoft.teams2", "microsoft-teams"),
    ("com.microsoft.OneDrive", "microsoft-onedrive"),
    ("com.microsoft.edgemac", "microsoft-edge"),
    ("com.microsoft.VSCode", "visual-studio-code"),
];

pub struct MicrosoftAutoUpdateChecker;

impl MicrosoftAutoUpdateChecker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UpdateChecker for MicrosoftAutoUpdateChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::MicrosoftAutoupdate
    }

    fn can_check(&self, bundle_id: &str, _app_path: &Path, install_source: &AppSource) -> bool {
        *install_source != AppSource::MacAppStore && microsoft_apps().contains_key(bundle_id)
    }

    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        log::info!("Microsoft: checking {} (path: {})", bundle_id, app_path.display());

        let current = match current_version {
            Some(v) => v,
            None => {
                log::info!("Microsoft: no current version for {}, skipping", bundle_id);
                return Ok(None);
            }
        };

        let app_key = match microsoft_apps().get(bundle_id) {
            Some(k) => *k,
            None => return Ok(None),
        };

        // 1) Try the macadmins.software XML feed
        if let Some(update) = check_macadmins_xml(bundle_id, app_key, current, client).await? {
            log::info!(
                "Microsoft: {} update found via macadmins XML: {} → {}",
                bundle_id, current, update.available_version
            );
            return Ok(Some(update));
        }

        // 2) Fallback to Homebrew cask index
        if let Some(ref index) = context.homebrew_cask_index {
            if let Some(cask_info) = index.lookup(bundle_id, app_path) {
                if version_compare::is_newer(current, &cask_info.version) {
                    log::info!(
                        "Microsoft (Homebrew fallback): {} has update {} -> {}",
                        bundle_id, current, cask_info.version
                    );
                    let release_notes_url = context.github_repo.as_ref()
                        .map(|slug| format!("https://github.com/{}/releases", slug))
                        .or_else(|| office_release_notes_url(bundle_id));
                    return Ok(Some(UpdateInfo {
                        bundle_id: bundle_id.to_string(),
                        current_version: Some(current.to_string()),
                        available_version: cask_info.version.clone(),
                        source_type: UpdateSourceType::MicrosoftAutoupdate,
                        download_url: None,
                        release_notes_url,
                        release_notes: None,
                        is_paid_upgrade: false,
                        notes: None,
                    }));
                }
            }
        }

        // 3) Hardcoded cask token fallback via brew outdated
        if let Some(ref outdated_map) = context.brew_outdated {
            let cask_token = context.homebrew_cask_token.as_deref()
                .or_else(|| lookup_hardcoded_token(bundle_id));

            if let Some(token) = cask_token {
                if let Some(outdated) = outdated_map.get(token) {
                    log::info!(
                        "Microsoft (brew outdated fallback): {} found via token '{}' (installed: {}, available: {})",
                        bundle_id, token, outdated.installed_versions, outdated.current_version
                    );
                    return Ok(Some(UpdateInfo {
                        bundle_id: bundle_id.to_string(),
                        current_version: Some(current.to_string()),
                        available_version: outdated.current_version.clone(),
                        source_type: UpdateSourceType::MicrosoftAutoupdate,
                        download_url: None,
                        release_notes_url: office_release_notes_url(bundle_id),
                        release_notes: None,
                        is_paid_upgrade: false,
                        notes: Some("Update available via Homebrew".to_string()),
                    }));
                }
            }
        }

        // 4) SHA-256 change detection as final fallback
        let cask_token = context.homebrew_cask_token.as_deref()
            .or_else(|| lookup_hardcoded_token(bundle_id));

        if let (Some(token), Some(ref db)) = (cask_token, &context.db) {
            match cask_sha_checker::check_cask_sha(token, client, db).await {
                CaskShaResult::Changed => {
                    log::info!("Microsoft: {} SHA changed — update likely available", bundle_id);
                    return Ok(Some(UpdateInfo {
                        bundle_id: bundle_id.to_string(),
                        current_version: Some(current.to_string()),
                        available_version: format!("{} (newer build)", current),
                        source_type: UpdateSourceType::MicrosoftAutoupdate,
                        download_url: None,
                        release_notes_url: office_release_notes_url(bundle_id),
                        release_notes: None,
                        is_paid_upgrade: false,
                        notes: Some("Update detected via cask SHA change".to_string()),
                    }));
                }
                CaskShaResult::Error(e) => {
                    log::info!("Microsoft: SHA check error for {}: {}", token, e);
                }
                _ => {}
            }
        }

        log::info!("Microsoft: {} is up to date ({})", bundle_id, current);
        Ok(None)
    }
}

/// Return a release notes URL for a known Microsoft app.
fn office_release_notes_url(bundle_id: &str) -> Option<String> {
    match bundle_id {
        "com.microsoft.Word" | "com.microsoft.Excel" | "com.microsoft.Powerpoint"
        | "com.microsoft.Outlook" | "com.microsoft.onenote.mac" =>
            Some("https://learn.microsoft.com/en-us/officeupdates/release-notes-office-for-mac".to_string()),
        "com.microsoft.teams2" | "com.microsoft.teams" =>
            Some("https://learn.microsoft.com/en-us/officeupdates/teams-app-versioning".to_string()),
        "com.microsoft.edgemac" =>
            Some("https://learn.microsoft.com/en-us/deployedge/microsoft-edge-relnote-stable-channel".to_string()),
        "com.microsoft.VSCode" =>
            Some("https://code.visualstudio.com/updates".to_string()),
        "com.microsoft.OneDrive" =>
            Some("https://support.microsoft.com/en-us/office/onedrive-release-notes-845dcf18-f921-435e-bf28-4e24b95e5fc0".to_string()),
        _ => None,
    }
}

/// Look up a hardcoded cask token for a Microsoft bundle ID.
pub fn lookup_hardcoded_token(bundle_id: &str) -> Option<&'static str> {
    MICROSOFT_CASK_TOKENS
        .iter()
        .find(|(bid, _)| *bid == bundle_id)
        .map(|(_, token)| *token)
}

async fn check_macadmins_xml(
    bundle_id: &str,
    app_key: &str,
    current: &str,
    client: &reqwest::Client,
) -> AppResult<Option<UpdateInfo>> {
    let latest_version = super::macadmins_feed::check_macadmins_version(app_key, bundle_id, client).await;

    if let Some(version) = latest_version {
        log::info!(
            "Microsoft AutoUpdate: {} (key: {}) current={} available={}",
            bundle_id, app_key, current, version
        );
        if version_compare::is_newer(current, &version) {
            log::info!(
                "Microsoft AutoUpdate: {} has update {} -> {}",
                bundle_id, current, version
            );
            return Ok(Some(UpdateInfo {
                bundle_id: bundle_id.to_string(),
                current_version: Some(current.to_string()),
                available_version: version,
                source_type: UpdateSourceType::MicrosoftAutoupdate,
                download_url: None,
                release_notes_url: office_release_notes_url(bundle_id),
                release_notes: None,
                is_paid_upgrade: false,
                notes: None,
            }));
        }
    } else {
        log::info!(
            "Microsoft AutoUpdate: no matching package found in XML for {} (key: {})",
            bundle_id, app_key
        );
    }

    Ok(None)
}
