use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

use super::version_compare;
use super::UpdateChecker;
use crate::detection::bundle_reader;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

pub struct MacAppStoreChecker;

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    #[serde(rename = "resultCount")]
    result_count: u32,
    results: Vec<ItunesResult>,
}

#[derive(Debug, Deserialize)]
struct ItunesResult {
    #[serde(rename = "bundleId")]
    bundle_id: Option<String>,
    version: String,
    #[serde(rename = "trackViewUrl")]
    track_view_url: Option<String>,
    #[serde(rename = "releaseNotes")]
    release_notes: Option<String>,
}

#[async_trait]
impl UpdateChecker for MacAppStoreChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::MacAppStore
    }

    fn can_check(&self, _bundle_id: &str, app_path: &Path, install_source: &AppSource) -> bool {
        *install_source == AppSource::MacAppStore || bundle_reader::has_mas_receipt(app_path)
    }

    async fn check(
        &self,
        bundle_id: &str,
        _app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        _context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let url = format!(
            "https://itunes.apple.com/lookup?bundleId={}&country=US",
            bundle_id
        );
        let resp = client.get(&url).send().await?;
        let data: ItunesResponse = resp.json().await?;

        if data.result_count == 0 || data.results.is_empty() {
            return Ok(None);
        }

        let result = &data.results[0];

        if let Some(current) = current_version {
            if version_compare::is_newer(current, &result.version) {
                return Ok(Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: Some(current.to_string()),
                    available_version: result.version.clone(),
                    source_type: UpdateSourceType::MacAppStore,
                    download_url: result.track_view_url.clone(),
                    release_notes_url: None,
                    release_notes: result.release_notes.clone(),
                    is_paid_upgrade: false,
                    notes: None,
                }));
            }
        }

        Ok(None)
    }
}
