use async_trait::async_trait;
use std::path::Path;

use super::version_compare;
use super::UpdateChecker;
use crate::detection::bundle_reader;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

pub struct ElectronChecker;

impl ElectronChecker {
    pub fn new() -> Self {
        Self
    }
}

/// Parsed electron-builder update configuration from app-update.yml.
struct ElectronUpdateConfig {
    provider: String,
    owner: Option<String>,
    repo: Option<String>,
    url: Option<String>,
}

/// Parse a simple YAML key-value file (electron-builder's app-update.yml is flat).
fn parse_update_yml(content: &str) -> Option<ElectronUpdateConfig> {
    let mut provider = None;
    let mut owner = None;
    let mut repo = None;
    let mut url = None;

    for line in content.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            match key {
                "provider" => provider = Some(value.to_string()),
                "owner" => owner = Some(value.to_string()),
                "repo" => repo = Some(value.to_string()),
                "url" => url = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Some(ElectronUpdateConfig {
        provider: provider?,
        owner,
        repo,
        url,
    })
}

#[async_trait]
impl UpdateChecker for ElectronChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::Electron
    }

    fn can_check(&self, _bundle_id: &str, app_path: &Path, _install_source: &AppSource) -> bool {
        bundle_reader::is_electron_app(app_path)
    }

    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        _context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let current = match current_version {
            Some(v) => v,
            None => return Ok(None),
        };

        let resources = app_path.join("Contents/Resources");

        // Try app-update.yml first, then dev-app-update.yml
        let yml_path = if resources.join("app-update.yml").exists() {
            resources.join("app-update.yml")
        } else if resources.join("dev-app-update.yml").exists() {
            resources.join("dev-app-update.yml")
        } else {
            return Ok(None);
        };

        let content = match std::fs::read_to_string(&yml_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let config = match parse_update_yml(&content) {
            Some(c) => c,
            None => return Ok(None),
        };

        match config.provider.as_str() {
            "github" => {
                let owner = match config.owner {
                    Some(ref o) => o.as_str(),
                    None => return Ok(None),
                };
                let repo = match config.repo {
                    Some(ref r) => r.as_str(),
                    None => return Ok(None),
                };

                // Delegate to existing GitHub release check logic
                let result = super::github_releases::check_github_release(
                    owner,
                    repo,
                    bundle_id,
                    Some(current),
                    client,
                ).await?;

                // Re-tag as Electron source
                Ok(result.map(|mut u| {
                    u.source_type = UpdateSourceType::Electron;
                    u
                }))
            }
            "generic" => {
                let base_url = match config.url {
                    Some(ref u) => u.trim_end_matches('/'),
                    None => return Ok(None),
                };

                // Fetch latest-mac.yml from the generic update server
                let yml_url = format!("{}/latest-mac.yml", base_url);
                let resp = match client.get(&yml_url).send().await {
                    Ok(r) => r,
                    Err(_) => return Ok(None),
                };

                if !resp.status().is_success() {
                    return Ok(None);
                }

                let body = resp.text().await?;

                // Parse version from latest-mac.yml (format: "version: X.Y.Z")
                let available = body
                    .lines()
                    .find(|l| l.trim().starts_with("version:"))
                    .and_then(|l| l.split_once(':'))
                    .map(|(_, v)| v.trim().trim_matches('"').trim_matches('\'').to_string());

                let available = match available {
                    Some(v) => v,
                    None => return Ok(None),
                };

                if version_compare::is_newer(current, &available) {
                    log::info!(
                        "Electron (generic): {} has update {} -> {}",
                        bundle_id, current, available
                    );
                    return Ok(Some(UpdateInfo {
                        bundle_id: bundle_id.to_string(),
                        current_version: Some(current.to_string()),
                        available_version: available,
                        source_type: UpdateSourceType::Electron,
                        download_url: None,
                        release_notes_url: None,
                        release_notes: None,
                        is_paid_upgrade: false,
                        notes: None,
                    }));
                }

                Ok(None)
            }
            // s3, spaces, and other providers require auth -- can't check
            _ => Ok(None),
        }
    }
}
