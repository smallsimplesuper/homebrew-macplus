use async_trait::async_trait;

use super::bundle_reader;
use super::AppDetector;
use crate::models::{AppSource, DetectedApp};
use crate::utils::command::run_command_with_timeout;
use crate::utils::{AppError, AppResult};

pub struct MasDetector;

async fn is_mas_installed() -> bool {
    run_command_with_timeout("mas", &["version"], 5).await.is_ok()
}

#[async_trait]
impl AppDetector for MasDetector {
    fn name(&self) -> &str {
        "Mac App Store"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        if !is_mas_installed().await {
            log::info!("mas-cli not installed, skipping");
            return Ok(Vec::new());
        }

        let output = run_command_with_timeout("mas", &["list"], 15).await
            .map_err(|e| AppError::CommandFailed(format!("mas list: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let apps: Vec<DetectedApp> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                // Format: "497799835  Xcode  (15.2)"
                let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
                if parts.len() < 2 {
                    return None;
                }

                let apple_id = parts[0].trim().to_string();
                let rest = parts[1].trim();

                let (name, version) = if let Some(pos) = rest.rfind('(') {
                    let name = rest[..pos].trim();
                    let ver = rest[pos + 1..].trim_end_matches(')').trim();
                    (name.to_string(), Some(ver.to_string()))
                } else {
                    (rest.to_string(), None)
                };

                if name.is_empty() {
                    return None;
                }

                // Try to resolve app path and bundle info
                let app_path = format!("/Applications/{}.app", name);
                let path = std::path::Path::new(&app_path);

                let (bundle_id, resolved_path, resolved_version, bundle_version, architectures, sparkle_feed_url) =
                    if path.exists() {
                        if let Some(bundle) = bundle_reader::read_bundle(path) {
                            (
                                bundle.bundle_id,
                                bundle.app_path,
                                bundle.installed_version.or(version.clone()),
                                bundle.bundle_version,
                                bundle.architectures,
                                bundle.sparkle_feed_url,
                            )
                        } else {
                            (String::new(), app_path, version.clone(), None, None, None)
                        }
                    } else {
                        (String::new(), String::new(), version.clone(), None, None, None)
                    };

                Some(DetectedApp {
                    bundle_id,
                    display_name: name,
                    app_path: resolved_path,
                    installed_version: resolved_version,
                    bundle_version,
                    install_source: AppSource::MacAppStore,
                    obtained_from: Some("mac_app_store".into()),
                    homebrew_cask_token: None,
                    architectures,
                    sparkle_feed_url,
                    mas_app_id: Some(apple_id),
                    homebrew_formula_name: None,
                })
            })
            .collect();

        Ok(apps)
    }
}
