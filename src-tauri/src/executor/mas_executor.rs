use std::process::Command;

use crate::models::UpdateResult;
use crate::utils::{AppError, AppResult};
use super::UpdateExecutor;

pub struct MasExecutor {
    pub mas_app_id: Option<String>,
}

impl MasExecutor {
    pub fn new(mas_app_id: Option<String>) -> Self {
        Self { mas_app_id }
    }
}

impl UpdateExecutor for MasExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        let app_id = self.mas_app_id.as_deref().ok_or_else(|| {
            AppError::NotFound(format!(
                "No MAS app ID found for {}. Cannot perform targeted upgrade.",
                bundle_id
            ))
        })?;

        // System-protected apps (e.g. Mail, Safari) live under /System/ and cannot
        // be updated via `mas upgrade` — macOS SIP blocks modification. Instead,
        // open the Mac App Store to the app's page so the user can update from there.
        if app_path.starts_with("/System/") {
            on_progress(0, "Opening Mac App Store for system app…", None);

            let url = format!("macappstore://apps.apple.com/app/id{}", app_id);
            let output = Command::new("open")
                .arg(&url)
                .output()
                .map_err(|e| AppError::CommandFailed(format!("Failed to open App Store: {}", e)))?;

            if output.status.success() {
                on_progress(100, "Opened Mac App Store", None);
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: true,
                    message: Some(format!("Opened Mac App Store for {}", bundle_id)),
                    source_type: "mas".to_string(),
                    from_version: None,
                    to_version: None,
                    handled_relaunch: false,
                delegated: false,
                });
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(AppError::CommandFailed(format!(
                    "Failed to open Mac App Store: {}",
                    stderr
                )));
            }
        }

        on_progress(0, &format!("Starting Mac App Store upgrade for app {}", app_id), None);

        let output = Command::new("mas")
            .current_dir("/tmp")
            .args(["upgrade", app_id])
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to run mas: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            on_progress(100, "MAS upgrade completed", None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: true,
                message: Some("Successfully upgraded via Mac App Store".to_string()),
                source_type: "mas".to_string(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            })
        } else {
            let error_msg = if stderr.is_empty() { stdout } else { stderr };
            let msg = format!("Mac App Store upgrade failed: {}", error_msg);
            on_progress(100, &msg, None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(format!("Failed to upgrade via Mac App Store: {}", error_msg)),
                source_type: "mas".to_string(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            })
        }
    }
}
