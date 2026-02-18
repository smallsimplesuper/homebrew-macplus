pub mod adobe_cc;
pub mod cask_sha_checker;
pub mod electron;
pub mod github_releases;
pub mod homebrew_api;
pub mod homebrew_cask;
pub mod homebrew_formula;
pub mod jetbrains_toolbox;
pub mod keystone;
pub mod mac_app_store;
pub mod macadmins_feed;
pub mod microsoft_autoupdate;
pub mod mozilla;
pub mod sparkle;
pub mod version_compare;

use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

/// Cached info from `brew outdated --cask --greedy --json=v2`
#[derive(Debug, Clone)]
pub struct BrewOutdatedCask {
    pub current_version: String,
    pub installed_versions: String,
}

/// Cached info from `brew outdated --formula --json=v2`
#[derive(Debug, Clone)]
pub struct BrewOutdatedFormula {
    pub current_version: String,
    pub installed_version: String,
}

pub struct AppCheckContext {
    pub homebrew_cask_token: Option<String>,
    pub sparkle_feed_url: Option<String>,
    pub obtained_from: Option<String>,
    pub brew_outdated: Option<Arc<HashMap<String, BrewOutdatedCask>>>,
    pub brew_outdated_formulae: Option<Arc<HashMap<String, BrewOutdatedFormula>>>,
    pub homebrew_cask_index: Option<Arc<homebrew_api::HomebrewCaskIndex>>,
    pub github_repo: Option<String>,
    pub homebrew_formula_name: Option<String>,
    /// Whether Xcode Command Line Tools are installed (checked once per cycle).
    pub xcode_clt_installed: Option<bool>,
    /// Database handle for cask SHA cache lookups.
    pub db: Option<Arc<Mutex<Database>>>,
}

#[async_trait]
pub trait UpdateChecker: Send + Sync {
    fn source_type(&self) -> UpdateSourceType;
    fn can_check(&self, bundle_id: &str, app_path: &Path, install_source: &AppSource) -> bool;
    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        context: &AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>>;
}

pub struct UpdateDispatcher {
    checkers: Vec<Box<dyn UpdateChecker>>,
}

impl UpdateDispatcher {
    pub fn new() -> Self {
        Self {
            checkers: vec![
                Box::new(sparkle::SparkleChecker),
                Box::new(homebrew_cask::HomebrewCaskChecker),
                Box::new(homebrew_api::HomebrewApiChecker),
                Box::new(mac_app_store::MacAppStoreChecker),
                Box::new(mozilla::MozillaChecker),
                Box::new(github_releases::GitHubReleasesChecker),
                Box::new(electron::ElectronChecker),
                Box::new(keystone::KeystoneChecker),
                Box::new(microsoft_autoupdate::MicrosoftAutoUpdateChecker),
                Box::new(jetbrains_toolbox::JetBrainsToolboxChecker),
                Box::new(adobe_cc::AdobeCCChecker),
                Box::new(homebrew_formula::HomebrewFormulaChecker),
            ],
        }
    }

    pub async fn check_update(
        &self,
        bundle_id: &str,
        app_path: &str,
        current_version: Option<&str>,
        install_source: &AppSource,
        client: &reqwest::Client,
        context: &AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let path = Path::new(app_path);

        // Re-read the on-disk version from the app bundle to avoid stale DB values
        let disk_version = crate::detection::bundle_reader::read_bundle(path)
            .and_then(|b| b.installed_version);
        let effective_version = disk_version.as_deref().or(current_version);

        // Collect applicable checkers
        let applicable: Vec<&dyn UpdateChecker> = self.checkers.iter()
            .filter(|c| c.can_check(bundle_id, path, install_source))
            .map(|c| c.as_ref())
            .collect();

        if applicable.is_empty() {
            log::info!("Update check for {}: no update found (tried: none)", bundle_id);
            return Ok(None);
        }

        // Partition into brew-local (sequential) and network-independent (concurrent) tiers.
        // Brew checkers share local cache and should run first sequentially.
        let mut brew_checkers: Vec<&dyn UpdateChecker> = Vec::new();
        let mut network_checkers: Vec<&dyn UpdateChecker> = Vec::new();

        for checker in &applicable {
            match checker.source_type() {
                UpdateSourceType::HomebrewCask
                | UpdateSourceType::HomebrewApi => brew_checkers.push(*checker),
                _ => network_checkers.push(*checker),
            }
        }

        let mut tried: Vec<String> = Vec::new();

        // Tier 1: Run brew checkers sequentially (they share brew cache)
        for checker in &brew_checkers {
            let source_name = checker.source_type().as_str().to_string();
            match checker.check(bundle_id, path, effective_version, client, context).await {
                Ok(Some(mut update)) => {
                    tried.push(source_name.clone());
                    log::info!(
                        "Update check for {}: {} → found {} (tried: {})",
                        bundle_id, source_name, update.available_version, tried.join(", ")
                    );
                    enrich_release_notes(&mut update, context, client).await;
                    return Ok(Some(update));
                }
                Ok(None) => { tried.push(source_name); }
                Err(e) => {
                    log::info!("Update check for {}: {} failed: {}", bundle_id, source_name, e);
                    tried.push(source_name);
                }
            }
        }

        // Tier 2: Run network checkers concurrently, return on first success
        if !network_checkers.is_empty() {
            let futures: Vec<_> = network_checkers.iter().map(|checker| {
                let source_name = checker.source_type().as_str().to_string();
                async move {
                    let result = checker.check(bundle_id, path, effective_version, client, context).await;
                    (source_name, result)
                }
            }).collect();

            let results = futures::future::join_all(futures).await;
            let mut found_update: Option<UpdateInfo> = None;
            for (source_name, result) in results {
                match result {
                    Ok(Some(update)) => {
                        if found_update.is_none() {
                            log::info!(
                                "Update check for {}: {} → found {}",
                                bundle_id, source_name, update.available_version
                            );
                            found_update = Some(update);
                        }
                        tried.push(source_name);
                    }
                    Ok(None) => { tried.push(source_name); }
                    Err(e) => {
                        log::info!("Update check for {}: {} failed: {}", bundle_id, source_name, e);
                        tried.push(source_name);
                    }
                }
            }
            if let Some(mut update) = found_update {
                enrich_release_notes(&mut update, context, client).await;
                return Ok(Some(update));
            }
        }

        let tried_str = if tried.is_empty() { "none".to_string() } else { tried.join(", ") };
        log::info!("Update check for {}: no update found (tried: {})", bundle_id, tried_str);

        Ok(None)
    }

    /// Run each checker individually and return diagnostic results for debugging.
    pub async fn debug_check(
        &self,
        bundle_id: &str,
        app_path: &str,
        current_version: Option<&str>,
        install_source: &AppSource,
        client: &reqwest::Client,
        context: &AppCheckContext,
    ) -> Vec<CheckerDiagnostic> {
        let path = Path::new(app_path);

        // Re-read the on-disk version from the app bundle to avoid stale DB values
        let disk_version = crate::detection::bundle_reader::read_bundle(path)
            .and_then(|b| b.installed_version);
        let effective_version = disk_version.as_deref().or(current_version);

        let mut results = Vec::new();

        for checker in &self.checkers {
            let source_name = checker.source_type().as_str().to_string();
            let can_check = checker.can_check(bundle_id, path, install_source);

            if !can_check {
                results.push(CheckerDiagnostic {
                    source: source_name,
                    can_check: false,
                    result: "skipped".to_string(),
                });
                continue;
            }

            let result_str = match checker.check(bundle_id, path, effective_version, client, context).await {
                Ok(Some(update)) => format!("found: {}", update.available_version),
                Ok(None) => "not_found".to_string(),
                Err(e) => format!("error: {}", e),
            };

            results.push(CheckerDiagnostic {
                source: source_name,
                can_check: true,
                result: result_str,
            });
        }

        results
    }
}

#[derive(Debug, Serialize)]
pub struct CheckerDiagnostic {
    pub source: String,
    pub can_check: bool,
    pub result: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateCheckDiagnostic {
    pub bundle_id: String,
    pub app_path: String,
    pub installed_version: Option<String>,
    pub install_source: String,
    pub homebrew_cask_token: Option<String>,
    pub checkers_tried: Vec<CheckerDiagnostic>,
}

/// Enrich an update with release notes if none were provided by the checker.
async fn enrich_release_notes(
    update: &mut UpdateInfo,
    context: &AppCheckContext,
    client: &reqwest::Client,
) {
    if update.release_notes.is_some() {
        // Sanitize existing notes
        if let Some(ref notes) = update.release_notes {
            update.release_notes = Some(crate::utils::sanitize::sanitize_release_notes(notes));
        }
        return;
    }

    // 1) GitHub: reuses ETag cache, no extra API call if already fetched
    if let Some(ref repo) = context.github_repo {
        if let Some(notes) = github_releases::fetch_release_notes(repo, client).await {
            update.release_notes = Some(crate::utils::sanitize::sanitize_release_notes(&notes));
            if update.release_notes_url.is_none() {
                update.release_notes_url = Some(format!("https://github.com/{}/releases", repo));
            }
            return;
        }
    }

    // 2) Sparkle: parse <description> from the appcast feed
    if let Some(ref feed_url) = context.sparkle_feed_url {
        if let Some(notes) = sparkle::fetch_sparkle_description(feed_url, client).await {
            update.release_notes = Some(crate::utils::sanitize::sanitize_release_notes(&notes));
        }
    }
}
