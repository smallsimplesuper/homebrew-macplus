use async_trait::async_trait;
use std::path::Path;

use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

pub struct HomebrewFormulaChecker;

#[async_trait]
impl UpdateChecker for HomebrewFormulaChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::HomebrewCask // Reuse for display; source_type string will be "homebrew_formula"
    }

    fn can_check(&self, _bundle_id: &str, _app_path: &Path, install_source: &AppSource) -> bool {
        *install_source == AppSource::HomebrewFormula
    }

    async fn check(
        &self,
        bundle_id: &str,
        _app_path: &Path,
        current_version: Option<&str>,
        _client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let formula_name = match &context.homebrew_formula_name {
            Some(name) => name,
            None => return Ok(None),
        };

        if let Some(ref outdated_map) = context.brew_outdated_formulae {
            if let Some(outdated) = outdated_map.get(formula_name.as_str()) {
                let notes = if context.xcode_clt_installed == Some(false) {
                    Some("Requires Xcode Command Line Tools (run: xcode-select --install)".to_string())
                } else {
                    None
                };

                return Ok(Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: current_version.map(String::from),
                    available_version: outdated.current_version.clone(),
                    source_type: UpdateSourceType::HomebrewCask, // Will be stored as source_type string
                    download_url: None,
                    release_notes_url: None,
                    release_notes: None,
                    is_paid_upgrade: false,
                    notes,
                }));
            }
        }

        Ok(None)
    }
}
