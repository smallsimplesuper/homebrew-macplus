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
pub mod microsoft_autoupdate;
pub mod mozilla;
pub mod sparkle;
pub mod version_compare;

use async_trait::async_trait;
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
            match checker.check(bundle_id, path, current_version, client, context).await {
                Ok(Some(update)) => {
                    tried.push(source_name.clone());
                    log::info!(
                        "Update check for {}: {} → found {} (tried: {})",
                        bundle_id, source_name, update.available_version, tried.join(", ")
                    );
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
                    let result = checker.check(bundle_id, path, current_version, client, context).await;
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
            if found_update.is_some() {
                return Ok(found_update);
            }
        }

        let tried_str = if tried.is_empty() { "none".to_string() } else { tried.join(", ") };
        log::info!("Update check for {}: no update found (tried: {})", bundle_id, tried_str);

        Ok(None)
    }
}
