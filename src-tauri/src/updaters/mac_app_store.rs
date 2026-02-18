use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

use super::version_compare;
use super::UpdateChecker;
use crate::detection::bundle_reader;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

/// Per-request timeout for iTunes API calls.
const ITUNES_TIMEOUT_SECS: u64 = 15;

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
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        _context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let url = format!(
            "https://itunes.apple.com/lookup?bundleId={}&country=US",
            bundle_id
        );

        let resp = match tokio::time::timeout(
            Duration::from_secs(ITUNES_TIMEOUT_SECS),
            client.get(&url).send(),
        ).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                log::info!("MAS checker: HTTP error for {}: {}", bundle_id, e);
                return Ok(None);
            }
            Err(_) => {
                log::info!("MAS checker: request timed out after {}s for {}", ITUNES_TIMEOUT_SECS, bundle_id);
                return Ok(None);
            }
        };

        let data: ItunesResponse = match resp.json().await {
            Ok(d) => d,
            Err(e) => {
                log::info!("MAS checker: failed to parse response for {}: {}", bundle_id, e);
                return Ok(None);
            }
        };

        if data.result_count == 0 || data.results.is_empty() {
            return Ok(None);
        }

        let result = &data.results[0];

        // Re-read the on-disk version to catch silent App Store updates
        let disk_version = bundle_reader::read_bundle(app_path)
            .and_then(|b| b.installed_version);

        // Prefer the fresh disk version over the database version
        let effective_version = disk_version.as_deref().or(current_version);

        if let Some(current) = effective_version {
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
