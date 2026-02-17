use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

pub struct MozillaChecker;

struct MozillaProduct {
    api_url: &'static str,
    version_key: &'static str,
}

fn mozilla_mappings() -> &'static HashMap<&'static str, MozillaProduct> {
    static MAPPINGS: OnceLock<HashMap<&str, MozillaProduct>> = OnceLock::new();
    MAPPINGS.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("org.mozilla.firefox", MozillaProduct {
            api_url: "https://product-details.mozilla.org/1.0/firefox_versions.json",
            version_key: "LATEST_FIREFOX_VERSION",
        });
        m.insert("org.mozilla.nightly", MozillaProduct {
            api_url: "https://product-details.mozilla.org/1.0/firefox_versions.json",
            version_key: "LATEST_FIREFOX_NIGHTLY_VERSION",
        });
        m.insert("org.mozilla.firefoxdeveloperedition", MozillaProduct {
            api_url: "https://product-details.mozilla.org/1.0/firefox_versions.json",
            version_key: "LATEST_FIREFOX_DEVEL_VERSION",
        });
        m.insert("org.mozilla.thunderbird", MozillaProduct {
            api_url: "https://product-details.mozilla.org/1.0/thunderbird_versions.json",
            version_key: "LATEST_THUNDERBIRD_VERSION",
        });
        m
    })
}

#[async_trait]
impl UpdateChecker for MozillaChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::Mozilla
    }

    fn can_check(&self, bundle_id: &str, _app_path: &Path, _install_source: &AppSource) -> bool {
        mozilla_mappings().contains_key(bundle_id)
    }

    async fn check(
        &self,
        bundle_id: &str,
        _app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        _context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let current = match current_version {
            Some(v) => v,
            None => return Ok(None),
        };

        let product = match mozilla_mappings().get(bundle_id) {
            Some(p) => p,
            None => return Ok(None),
        };

        let resp = client.get(product.api_url).send().await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let versions: HashMap<String, serde_json::Value> = resp.json().await?;

        let available = match versions.get(product.version_key).and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(None),
        };

        if version_compare::is_newer(current, available) {
            log::info!(
                "Mozilla: {} has update {} -> {}",
                bundle_id, current, available
            );
            return Ok(Some(UpdateInfo {
                bundle_id: bundle_id.to_string(),
                current_version: Some(current.to_string()),
                available_version: available.to_string(),
                source_type: UpdateSourceType::Mozilla,
                download_url: None,
                release_notes_url: Some(format!(
                    "https://www.mozilla.org/en-US/firefox/{}/releasenotes/",
                    available
                )),
                release_notes: None,
                is_paid_upgrade: false,
                notes: None,
            }));
        }

        Ok(None)
    }
}
