use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::cask_sha_checker::{self, CaskShaResult};
use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::http_client::APP_USER_AGENT;
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
const MICROSOFT_CASK_TOKENS: &[(&str, &str)] = &[
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

    fn can_check(&self, bundle_id: &str, _app_path: &Path, _install_source: &AppSource) -> bool {
        microsoft_apps().contains_key(bundle_id)
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
                        .map(|slug| format!("https://github.com/{}/releases", slug));
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
                        release_notes_url: None,
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
                        release_notes_url: None,
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

/// Look up a hardcoded cask token for a Microsoft bundle ID.
fn lookup_hardcoded_token(bundle_id: &str) -> Option<&'static str> {
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
    let resp = client
        .get("https://macadmins.software/latest.xml")
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .await?;

    if !resp.status().is_success() {
        log::info!(
            "Microsoft AutoUpdate: macadmins.software XML fetch returned status {} for {}",
            resp.status(),
            bundle_id
        );
        return Ok(None);
    }

    let xml_text = resp.text().await?;

    // Parse XML to find the version for the matching app
    let latest_version = extract_version_from_xml(&xml_text, app_key, bundle_id);

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
                release_notes_url: None,
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

/// Extract the latest version for a given app key from the macadmins.software XML.
/// The XML contains elements like <package> with <title> and <version> children.
/// Also tries matching by <cfbundleidentifier> as fallback.
fn extract_version_from_xml(xml: &str, app_key: &str, bundle_id: &str) -> Option<String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let app_key_lower = app_key.to_lowercase();
    let bundle_id_lower = bundle_id.to_lowercase();
    let mut in_package = false;
    let mut current_title = String::new();
    let mut current_version = String::new();
    let mut current_cfbundle = String::new();
    let mut reading_title = false;
    let mut reading_version = false;
    let mut reading_cfbundle = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "package" => {
                        in_package = true;
                        current_title.clear();
                        current_version.clear();
                        current_cfbundle.clear();
                    }
                    "title" if in_package => reading_title = true,
                    "version" if in_package => reading_version = true,
                    "cfbundleidentifier" if in_package => reading_cfbundle = true,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if reading_title {
                    current_title = e.decode().unwrap_or_default().trim().to_string();
                    reading_title = false;
                } else if reading_version {
                    current_version = e.decode().unwrap_or_default().trim().to_string();
                    reading_version = false;
                } else if reading_cfbundle {
                    current_cfbundle = e.decode().unwrap_or_default().trim().to_string();
                    reading_cfbundle = false;
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "package" && in_package {
                    // Match by title (primary) or by cfbundleidentifier (fallback)
                    let title_match = current_title.to_lowercase().contains(&app_key_lower);
                    let bundle_match = !current_cfbundle.is_empty()
                        && current_cfbundle.to_lowercase() == bundle_id_lower;

                    if (title_match || bundle_match) && !current_version.is_empty() {
                        return Some(current_version.clone());
                    }
                    in_package = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    None
}
