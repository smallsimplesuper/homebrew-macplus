use async_trait::async_trait;
use std::path::PathBuf;

use super::AppDetector;
use crate::models::{AppSource, DetectedApp};
use crate::utils::brew::brew_path;
use crate::utils::command::run_command_with_timeout;
use crate::utils::{AppError, AppResult};

/// Use Spotlight (`mdfind`) to find an app by its filename.
async fn find_app_by_name(app_name: &str) -> Option<PathBuf> {
    let output = run_command_with_timeout(
        "mdfind",
        &["kMDItemFSName ==", app_name, "-onlyin", "/Applications"],
        15,
    )
    .await
    .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

/// Cask tokens for macOS system components that Homebrew tracks but cannot
/// actually update. These must never appear in the app list or update UI.
const SYSTEM_CASK_BLOCKLIST: &[&str] = &[
    "toolreleases", // "System Events" — managed by macOS
];

pub struct HomebrewDetector;

#[async_trait]
impl AppDetector for HomebrewDetector {
    fn name(&self) -> &str {
        "Homebrew"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        let brew = match brew_path() {
            Some(p) => p,
            None => {
                log::info!("Homebrew not found, skipping detection");
                return Ok(Vec::new());
            }
        };

        let brew_str = brew.to_string_lossy().to_string();

        let output = run_command_with_timeout(&brew_str, &["list", "--cask"], 30)
            .await
            .map_err(|e| AppError::CommandFailed(format!("brew list --cask: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let cask_tokens: Vec<String> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();

        if cask_tokens.is_empty() {
            return Ok(Vec::new());
        }

        // Get JSON info for all casks at once
        let mut info_args: Vec<&str> = vec!["info", "--json=v2", "--cask"];
        let token_refs: Vec<&str> = cask_tokens.iter().map(|s| s.as_str()).collect();
        info_args.extend(&token_refs);

        let info_output = run_command_with_timeout(&brew_str, &info_args, 30)
            .await
            .map_err(|e| AppError::CommandFailed(format!("brew info: {}", e)))?;

        if !info_output.status.success() {
            return Ok(Vec::new());
        }

        let json: serde_json::Value = match serde_json::from_slice(&info_output.stdout) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Failed to parse brew info JSON: {}", e);
                return Ok(Vec::new());
            }
        };

        let casks = json["casks"].as_array().cloned().unwrap_or_default();
        let mut apps = Vec::new();

        for cask in &casks {
            let token = cask["token"].as_str().unwrap_or_default();
            if SYSTEM_CASK_BLOCKLIST.contains(&token) {
                continue;
            }
            let name = cask["name"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or(token);
            let version = cask["version"].as_str().unwrap_or_default();

            // Try to find the app path from artifacts
            let app_name = cask["artifacts"]
                .as_array()
                .and_then(|artifacts| {
                    artifacts.iter().find_map(|a| {
                        a.get("app")
                            .and_then(|app| app.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|v| v.as_str())
                    })
                })
                .unwrap_or_default();

            if app_name.is_empty() {
                // CLI-only cask (no .app artifact, e.g. docker) — track like a formula
                log::info!(
                    "Homebrew: detected CLI-only cask '{}' ({}), latest: {}",
                    token, name, version
                );
                let installed_version = cask["installed"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        // Some cask JSON uses an array of installed versions
                        cask["installed_versions"]
                            .as_array()
                            .and_then(|arr| arr.first())
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| version.to_string());

                apps.push(DetectedApp {
                    bundle_id: format!("homebrew.cask.{}", token),
                    display_name: name.to_string(),
                    app_path: String::new(),
                    installed_version: Some(installed_version),
                    bundle_version: None,
                    install_source: AppSource::Homebrew,
                    obtained_from: Some("homebrew".into()),
                    homebrew_cask_token: Some(token.to_string()),
                    architectures: None,
                    sparkle_feed_url: None,
                    mas_app_id: None,
                    homebrew_formula_name: None,
                });
                continue;
            }

            // Try standard /Applications path first, then ~/Applications, then mdfind
            let app_path = format!("/Applications/{}", app_name);
            let path = std::path::Path::new(&app_path);

            let resolved_path = if path.exists() {
                path.to_path_buf()
            } else {
                // Try ~/Applications
                let home_path = dirs::home_dir()
                    .map(|h| h.join("Applications").join(app_name));
                if let Some(ref hp) = home_path {
                    if hp.exists() {
                        hp.clone()
                    } else {
                        // Use mdfind (Spotlight) to find the app by name
                        find_app_by_name(app_name).await.unwrap_or_default()
                    }
                } else {
                    find_app_by_name(app_name).await.unwrap_or_default()
                }
            };

            if !resolved_path.exists() {
                continue;
            }

            let path = &resolved_path;

            // Read bundle info for the full details
            if let Some(bundle) =
                super::bundle_reader::read_bundle(path)
            {
                apps.push(DetectedApp {
                    bundle_id: bundle.bundle_id,
                    display_name: bundle.display_name,
                    app_path: bundle.app_path,
                    installed_version: bundle
                        .installed_version
                        .or_else(|| Some(version.to_string())),
                    bundle_version: bundle.bundle_version,
                    install_source: AppSource::Homebrew,
                    obtained_from: Some("homebrew".into()),
                    homebrew_cask_token: Some(token.to_string()),
                    architectures: bundle.architectures,
                    sparkle_feed_url: bundle.sparkle_feed_url,
                    mas_app_id: None,
                    homebrew_formula_name: None,
                });
            }
        }

        Ok(apps)
    }
}
