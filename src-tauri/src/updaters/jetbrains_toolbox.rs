use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::http_client::APP_USER_AGENT;
use crate::utils::AppResult;

/// Maps bundle IDs to JetBrains product codes used by the releases API.
fn jetbrains_product_codes() -> &'static HashMap<&'static str, &'static str> {
    static CODES: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    CODES.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("com.jetbrains.intellij", "IIU");
        m.insert("com.jetbrains.intellij.ce", "IIC");
        m.insert("com.jetbrains.WebStorm", "WS");
        m.insert("com.jetbrains.PhpStorm", "PS");
        m.insert("com.jetbrains.CLion", "CL");
        m.insert("com.jetbrains.goland", "GO");
        m.insert("com.jetbrains.rider", "RD");
        m.insert("com.jetbrains.pycharm", "PY");
        m.insert("com.jetbrains.pycharm.ce", "PC");
        m.insert("com.jetbrains.rubymine", "RM");
        m.insert("com.jetbrains.datagrip", "DG");
        m.insert("com.jetbrains.fleet", "FL");
        m.insert("com.jetbrains.toolbox", "TBA");
        m
    })
}

pub struct JetBrainsToolboxChecker;

impl JetBrainsToolboxChecker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UpdateChecker for JetBrainsToolboxChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::JetbrainsToolbox
    }

    fn can_check(&self, bundle_id: &str, _app_path: &Path, _install_source: &AppSource) -> bool {
        jetbrains_product_codes().contains_key(bundle_id)
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

        let product_code = match jetbrains_product_codes().get(bundle_id) {
            Some(c) => *c,
            None => return Ok(None),
        };

        let url = format!(
            "https://data.services.jetbrains.com/products/releases?code={}&latest=true&type=release",
            product_code
        );

        let resp = client
            .get(&url)
            .header("User-Agent", APP_USER_AGENT)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = resp.json().await?;

        // Response is { "CODE": [ { "version": "...", ... } ] }
        let version = json
            .get(product_code)
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .and_then(|rel| rel.get("version"))
            .and_then(|v| v.as_str());

        let download_url = json
            .get(product_code)
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .and_then(|rel| rel.get("downloads"))
            .and_then(|dl| dl.get("mac"))
            .and_then(|mac| mac.get("link"))
            .and_then(|v| v.as_str())
            .map(String::from);

        if let Some(latest) = version {
            if version_compare::is_newer(current, latest) {
                log::info!(
                    "JetBrains: {} has update {} -> {} ({})",
                    bundle_id, current, latest, product_code
                );
                return Ok(Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: Some(current.to_string()),
                    available_version: latest.to_string(),
                    source_type: UpdateSourceType::JetbrainsToolbox,
                    download_url,
                    release_notes_url: None,
                    release_notes: None,
                    is_paid_upgrade: false,
                    notes: None,
                }));
            }
        }

        Ok(None)
    }
}
