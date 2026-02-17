use async_trait::async_trait;

use super::bundle_reader;
use super::AppDetector;
use crate::models::{AppSource, DetectedApp};
use crate::utils::command::run_command_with_timeout;
use crate::utils::{AppError, AppResult};

pub struct SystemProfilerDetector;

#[async_trait]
impl AppDetector for SystemProfilerDetector {
    fn name(&self) -> &str {
        "System Profiler"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        let output = run_command_with_timeout(
            "system_profiler",
            &["SPApplicationsDataType", "-json"],
            30,
        )
        .await
        .map_err(|e| AppError::CommandFailed(format!("system_profiler: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::CommandFailed(
                "system_profiler returned non-zero".into(),
            ));
        }

        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_default();

        let items = json["SPApplicationsDataType"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let apps: Vec<DetectedApp> = items
            .iter()
            .filter_map(|item| {
                let path_str = item["path"].as_str()?;
                let path = std::path::Path::new(path_str);

                if !path.exists() || !path_str.ends_with(".app") {
                    return None;
                }

                // Skip system internals
                if path_str.starts_with("/System/Library/") {
                    return None;
                }

                let bundle = bundle_reader::read_bundle(path)?;

                let obtained_from = item["obtained_from"].as_str().map(String::from);
                let source = match obtained_from.as_deref() {
                    Some("mac_app_store") => AppSource::MacAppStore,
                    Some("identified_developer") => AppSource::Direct,
                    Some("apple") => AppSource::Direct,
                    _ => bundle_reader::detect_install_source(path),
                };

                let arch = item["arch_kind"].as_str().map(|a| {
                    match a {
                        "arch_arm_i64" => vec!["arm64".to_string(), "x86_64".to_string()],
                        "arch_arm" => vec!["arm64".to_string()],
                        "arch_i64" => vec!["x86_64".to_string()],
                        _ => vec![a.to_string()],
                    }
                });

                Some(DetectedApp {
                    bundle_id: bundle.bundle_id,
                    display_name: bundle.display_name,
                    app_path: bundle.app_path,
                    installed_version: bundle.installed_version,
                    bundle_version: bundle.bundle_version,
                    install_source: source,
                    obtained_from,
                    homebrew_cask_token: None,
                    architectures: arch.or(bundle.architectures),
                    sparkle_feed_url: bundle.sparkle_feed_url,
                    mas_app_id: None,
                    homebrew_formula_name: None,
                })
            })
            .collect();

        Ok(apps)
    }
}
