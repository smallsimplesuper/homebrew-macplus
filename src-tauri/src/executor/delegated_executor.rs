use std::process::Command;

use crate::models::UpdateResult;
use crate::utils::{AppError, AppResult};
use super::UpdateExecutor;

pub struct DelegatedExecutor;

impl DelegatedExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl UpdateExecutor for DelegatedExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        let msg = format!("Opening {} to trigger self-update", app_path);
        on_progress(0, &msg, None);

        let output = Command::new("open")
            .current_dir("/tmp")
            .arg(app_path)
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to open app: {}", e)))?;

        if output.status.success() {
            on_progress(100, "App opened for self-update", None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: true,
                message: Some(format!(
                    "Opened {}. The app will handle updating itself (e.g. via Sparkle).",
                    app_path
                )),
                source_type: "sparkle".to_string(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: true,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let msg = format!("Failed to open app: {}", stderr);
            on_progress(100, &msg, None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(format!("Failed to open {}: {}", app_path, stderr)),
                source_type: "sparkle".to_string(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: true,
            })
        }
    }
}
