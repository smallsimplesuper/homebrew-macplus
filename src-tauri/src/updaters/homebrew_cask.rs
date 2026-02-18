use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::{version_compare, BrewOutdatedCask, BrewOutdatedFormula, UpdateChecker};
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::brew::brew_path;
use crate::utils::AppResult;

/// Cask tokens for macOS system components that Homebrew tracks but cannot
/// actually update. Filtered out of the outdated map as a safety net.
const SYSTEM_CASK_BLOCKLIST: &[&str] = &["toolreleases"];

pub struct HomebrewCaskChecker;

#[async_trait]
impl UpdateChecker for HomebrewCaskChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::HomebrewCask
    }

    fn can_check(&self, _bundle_id: &str, _app_path: &Path, install_source: &AppSource) -> bool {
        // Don't check MAS apps via Homebrew
        *install_source != AppSource::MacAppStore
    }

    async fn check(
        &self,
        bundle_id: &str,
        _app_path: &Path,
        current_version: Option<&str>,
        _client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let cask_token = match &context.homebrew_cask_token {
            Some(token) => token,
            None => {
                log::debug!("No cask token for {}, skipping Homebrew Cask check", bundle_id);
                return Ok(None);
            }
        };

        // Look up direct download URL from the Homebrew API cask index
        let download_url = context.homebrew_cask_index
            .as_ref()
            .and_then(|idx| idx.url_by_token.get(cask_token.as_str()))
            .cloned();

        // Use pre-computed brew outdated map if available
        if let Some(ref outdated_map) = context.brew_outdated {
            if let Some(outdated) = outdated_map.get(cask_token.as_str()) {
                let release_notes_url = context.github_repo.as_ref()
                    .map(|slug| format!("https://github.com/{}/releases", slug));
                return Ok(Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: current_version.map(String::from),
                    available_version: outdated.current_version.clone(),
                    source_type: UpdateSourceType::HomebrewCask,
                    download_url,
                    release_notes_url,
                    release_notes: None,
                    is_paid_upgrade: false,
                    notes: None,
                }));
            }
            // Cask token exists but not in outdated list — up to date
            return Ok(None);
        }

        // Fallback: no pre-computed map (shouldn't happen in normal flow)
        log::debug!("No brew outdated cache for {}, skipping", bundle_id);
        Ok(None)
    }
}

/// Runs `brew outdated --cask --greedy --json=v2` once and returns a map of
/// cask token → outdated info. This should be called once per update-check cycle.
///
/// Uses flexible `serde_json::Value` parsing to handle Homebrew format changes gracefully.
pub fn fetch_brew_outdated() -> HashMap<String, BrewOutdatedCask> {
    let brew = match brew_path() {
        Some(p) => p,
        None => {
            log::info!("Homebrew not found, skipping brew outdated");
            return HashMap::new();
        }
    };

    let output = match Command::new(brew)
        .args(["outdated", "--cask", "--greedy", "--json=v2"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Failed to run `brew outdated`: {}", e);
            return HashMap::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("brew outdated failed: {}", stderr);
        return HashMap::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            let preview: String = stdout.chars().take(200).collect();
            log::warn!("Failed to parse brew outdated JSON: {}. Preview: {}", e, preview);
            return HashMap::new();
        }
    };

    let casks = match json.get("casks").and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => {
            log::warn!("brew outdated JSON has no 'casks' array");
            return HashMap::new();
        }
    };

    let mut map = HashMap::new();
    for c in casks {
        // Try "token" first (future-proof), fall back to "name"
        let token = c.get("token")
            .and_then(|v| v.as_str())
            .or_else(|| c.get("name").and_then(|v| v.as_str()));

        let token = match token {
            Some(t) if !SYSTEM_CASK_BLOCKLIST.contains(&t) => t.to_string(),
            Some(_) => continue, // system cask, skip
            None => continue,
        };

        let current_version = version_compare::strip_brew_version_token(
            c.get("current_version")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        ).to_string();

        // Handle installed_versions as array of strings gracefully
        let installed_versions = c.get("installed_versions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        map.insert(token, BrewOutdatedCask {
            current_version,
            installed_versions,
        });
    }

    map
}

/// Runs `brew outdated --formula --json=v2` once and returns a map of
/// formula name → outdated info.
pub fn fetch_brew_outdated_formulae() -> HashMap<String, BrewOutdatedFormula> {
    let brew = match brew_path() {
        Some(p) => p,
        None => return HashMap::new(),
    };

    let output = match Command::new(brew)
        .args(["outdated", "--formula", "--json=v2"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Failed to run `brew outdated --formula`: {}", e);
            return HashMap::new();
        }
    };

    if !output.status.success() {
        return HashMap::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let formulae = match json.get("formulae").and_then(|f| f.as_array()) {
        Some(arr) => arr,
        None => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for f in formulae {
        let name = f
            .get("name")
            .and_then(|v| v.as_str());

        let name = match name {
            Some(n) => n.to_string(),
            None => continue,
        };

        let current_version = f
            .get("current_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let installed_version = f
            .get("installed_versions")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        map.insert(name, BrewOutdatedFormula {
            current_version,
            installed_version,
        });
    }

    map
}
