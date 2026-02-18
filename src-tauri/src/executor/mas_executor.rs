use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::detection::bundle_reader;
use crate::models::UpdateResult;
use crate::utils::{AppError, AppResult};
use super::UpdateExecutor;

/// Timeout for `mas upgrade` commands (seconds).
const MAS_TIMEOUT_SECS: u64 = 120;

pub struct MasExecutor {
    pub mas_app_id: Option<String>,
    pre_version: Option<String>,
}

impl MasExecutor {
    pub fn new(mas_app_id: Option<String>) -> Self {
        Self { mas_app_id, pre_version: None }
    }

    pub fn with_pre_version(mut self, version: Option<String>) -> Self {
        self.pre_version = version;
        self
    }

    /// Check whether `mas` CLI is installed and available.
    fn mas_available() -> bool {
        Command::new("which")
            .arg("mas")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Detect whether stderr indicates a permission/elevation error.
    fn needs_elevation(stderr: &str) -> bool {
        stderr.contains("installd")
            || stderr.contains("PKInstallErrorDomain")
            || stderr.contains("Operation not permitted")
            || stderr.contains("connection to the installation service")
            || stderr.contains("Permission denied")
    }
}

impl UpdateExecutor for MasExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        // System-protected apps (e.g. Mail, Safari) live under /System/ and cannot
        // be updated via `mas upgrade` — macOS SIP blocks modification. Delegate
        // to the App Store directly.
        if app_path.starts_with("/System/") {
            return self.delegate_to_app_store(bundle_id, on_progress);
        }

        // Read pre-install version from the app bundle
        let pre_version = self.pre_version.clone().or_else(|| {
            bundle_reader::read_bundle(Path::new(app_path))
                .and_then(|b| b.installed_version)
        });

        let app_id = match self.mas_app_id.as_deref() {
            Some(id) => id.to_string(),
            None => {
                log::info!("MAS executor: no app ID for {}, delegating to App Store", bundle_id);
                return self.delegate_to_app_store(bundle_id, on_progress);
            }
        };

        // Skip Tier 1 entirely if `mas` isn't installed
        if !Self::mas_available() {
            log::info!("MAS executor: mas CLI not found, delegating to App Store for {}", bundle_id);
            return self.delegate_to_app_store_with_id(&app_id, bundle_id, &pre_version, on_progress);
        }

        // === Tier 1a: Try `mas upgrade` without elevation ===
        on_progress(0, &format!("Starting Mac App Store upgrade for app {}", app_id), None);
        log::info!("MAS executor: Tier 1a — trying mas upgrade {} (no elevation)", app_id);

        let tier1a_app_id = app_id.clone();
        let tier1a_result = tokio::time::timeout(
            Duration::from_secs(MAS_TIMEOUT_SECS),
            tokio::task::spawn_blocking(move || {
                Command::new("mas")
                    .current_dir("/tmp")
                    .args(["upgrade", &tier1a_app_id])
                    .output()
            }),
        ).await;

        match tier1a_result {
            Ok(Ok(Ok(output))) if output.status.success() => {
                on_progress(50, "mas upgrade completed, verifying...", None);
                log::info!("MAS executor: Tier 1a — mas upgrade exited 0 for {}", bundle_id);

                // Verify version actually changed
                let new_version = bundle_reader::read_bundle(Path::new(app_path))
                    .and_then(|b| b.installed_version);

                let changed = match (&pre_version, &new_version) {
                    (Some(old), Some(new)) => old != new,
                    _ => true,
                };

                if changed {
                    on_progress(100, "Mac App Store upgrade completed", None);
                    return Ok(UpdateResult {
                        bundle_id: bundle_id.to_string(),
                        success: true,
                        message: Some("Successfully upgraded via Mac App Store".to_string()),
                        source_type: "mas".to_string(),
                        from_version: pre_version,
                        to_version: new_version,
                        handled_relaunch: false,
                        delegated: false,
                    });
                }

                log::info!("MAS executor: Tier 1a — version unchanged for {}, trying Tier 1b", bundle_id);
            }
            Ok(Ok(Ok(output))) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                log::info!(
                    "MAS executor: Tier 1a failed for {} (exit {}): {}",
                    bundle_id,
                    output.status.code().unwrap_or(-1),
                    if stderr.is_empty() { &stdout } else { &stderr }
                );

                // If it's a permission error, try with elevation
                if Self::needs_elevation(&stderr) || Self::needs_elevation(&stdout) {
                    log::info!("MAS executor: Tier 1a detected elevation needed, trying Tier 1b");
                } else {
                    // Non-permission error — still try Tier 1b, it might help
                    log::info!("MAS executor: Tier 1a non-permission failure, trying Tier 1b anyway");
                }
            }
            Ok(Ok(Err(e))) => {
                log::info!("MAS executor: Tier 1a — failed to run mas for {}: {}", bundle_id, e);
            }
            Ok(Err(e)) => {
                log::info!("MAS executor: Tier 1a — spawn_blocking error for {}: {}", bundle_id, e);
            }
            Err(_) => {
                log::info!("MAS executor: Tier 1a — timed out after {}s for {}", MAS_TIMEOUT_SECS, bundle_id);
            }
        }

        // === Tier 1b: Retry with sudo elevation ===
        on_progress(10, "Retrying with administrator privileges...", None);
        log::info!("MAS executor: Tier 1b — trying elevated mas upgrade {} ", app_id);

        let tier1b_app_id = app_id.clone();
        let tier1b_result = tokio::time::timeout(
            Duration::from_secs(MAS_TIMEOUT_SECS),
            tokio::task::spawn_blocking(move || {
                crate::utils::sudo_session::run_elevated("mas", &["upgrade", &tier1b_app_id])
            }),
        ).await;

        match tier1b_result {
            Ok(Ok(Ok(output))) if output.status.success() => {
                on_progress(50, "Elevated mas upgrade completed, verifying...", None);
                log::info!("MAS executor: Tier 1b — elevated mas upgrade exited 0 for {}", bundle_id);

                let new_version = bundle_reader::read_bundle(Path::new(app_path))
                    .and_then(|b| b.installed_version);

                let changed = match (&pre_version, &new_version) {
                    (Some(old), Some(new)) => old != new,
                    _ => true,
                };

                if changed {
                    on_progress(100, "Mac App Store upgrade completed (elevated)", None);
                    return Ok(UpdateResult {
                        bundle_id: bundle_id.to_string(),
                        success: true,
                        message: Some("Successfully upgraded via Mac App Store (with admin privileges)".to_string()),
                        source_type: "mas".to_string(),
                        from_version: pre_version,
                        to_version: new_version,
                        handled_relaunch: false,
                        delegated: false,
                    });
                }

                log::info!("MAS executor: Tier 1b — version unchanged for {}, falling back to App Store", bundle_id);
            }
            Ok(Ok(Ok(output))) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                log::info!(
                    "MAS executor: Tier 1b failed for {} (exit {}): {}",
                    bundle_id,
                    output.status.code().unwrap_or(-1),
                    stderr.trim()
                );
            }
            Ok(Ok(Err(crate::utils::sudo_session::ElevatedError::UserCancelled))) => {
                log::info!("MAS executor: Tier 1b — user cancelled elevation for {}", bundle_id);
                // User cancelled — still fall through to App Store delegation
            }
            Ok(Ok(Err(e))) => {
                log::info!("MAS executor: Tier 1b — elevation error for {}: {}", bundle_id, e);
            }
            Ok(Err(e)) => {
                log::info!("MAS executor: Tier 1b — spawn_blocking error for {}: {}", bundle_id, e);
            }
            Err(_) => {
                log::info!("MAS executor: Tier 1b — timed out after {}s for {}", MAS_TIMEOUT_SECS, bundle_id);
            }
        }

        // === Tier 2: Fall back to App Store delegation ===
        on_progress(80, "Opening Mac App Store...", None);
        log::info!("MAS executor: Tier 2 — delegating to App Store for {}", bundle_id);
        self.delegate_to_app_store_with_id(&app_id, bundle_id, &pre_version, on_progress)
    }
}

impl MasExecutor {
    /// Open the Mac App Store to the specific app's page and return a delegated result.
    fn delegate_to_app_store_with_id(
        &self,
        app_id: &str,
        bundle_id: &str,
        pre_version: &Option<String>,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        let url = format!("macappstore://apps.apple.com/app/id{}", app_id);
        let output = Command::new("open")
            .arg(&url)
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to open App Store: {}", e)))?;

        if output.status.success() {
            on_progress(100, "Opened Mac App Store", None);
            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: true,
                message: Some(format!("Opened Mac App Store for {}", bundle_id)),
                source_type: "mas".to_string(),
                from_version: pre_version.clone(),
                to_version: None,
                handled_relaunch: false,
                delegated: true,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(AppError::CommandFailed(format!(
                "Failed to open Mac App Store: {}",
                stderr
            )))
        }
    }

    /// Open the Mac App Store (updates page or specific app) without a known app ID.
    fn delegate_to_app_store(
        &self,
        bundle_id: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        on_progress(0, "Opening Mac App Store...", None);

        let url = match self.mas_app_id.as_deref() {
            Some(id) => format!("macappstore://apps.apple.com/app/id{}", id),
            None => "macappstore://showUpdatesPage".to_string(),
        };

        let output = Command::new("open")
            .arg(&url)
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to open App Store: {}", e)))?;

        if output.status.success() {
            on_progress(100, "Opened Mac App Store", None);
            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: true,
                message: Some(format!("Opened Mac App Store for {}", bundle_id)),
                source_type: "mas".to_string(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: true,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(AppError::CommandFailed(format!(
                "Failed to open Mac App Store: {}",
                stderr
            )))
        }
    }
}
