use async_trait::async_trait;
use std::path::Path;

use super::bundle_reader;
use super::AppDetector;
use crate::models::DetectedApp;
use crate::utils::command::run_command_with_timeout;
use crate::utils::{AppError, AppResult};

pub struct SpotlightDetector;

#[async_trait]
impl AppDetector for SpotlightDetector {
    fn name(&self) -> &str {
        "Spotlight"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        let output = run_command_with_timeout(
            "mdfind",
            &["kMDItemContentType == 'com.apple.application-bundle'"],
            5,
        )
        .await
        .map_err(|e| AppError::CommandFailed(format!("mdfind: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::CommandFailed("mdfind returned non-zero".into()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let apps: Vec<DetectedApp> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter(|line| {
                let path = Path::new(line);
                path.extension().map_or(false, |ext| ext == "app") && path.exists()
            })
            .filter(|line| {
                // Skip system/internal apps that aren't user-visible
                !line.contains("/Contents/")
                    && !line.contains("/Library/Apple/")
                    && !line.starts_with("/System/Library/")
                    && !line.starts_with("/System/Applications/")
            })
            .filter_map(|line| {
                let app_path = Path::new(line);
                let bundle = bundle_reader::read_bundle(app_path)?;
                let source = bundle_reader::detect_install_source(app_path);

                Some(DetectedApp {
                    bundle_id: bundle.bundle_id,
                    display_name: bundle.display_name,
                    app_path: bundle.app_path,
                    installed_version: bundle.installed_version,
                    bundle_version: bundle.bundle_version,
                    install_source: source,
                    obtained_from: None,
                    homebrew_cask_token: None,
                    architectures: bundle.architectures,
                    sparkle_feed_url: bundle.sparkle_feed_url,
                    mas_app_id: None,
                    homebrew_formula_name: None,
                })
            })
            .collect();

        Ok(apps)
    }
}
