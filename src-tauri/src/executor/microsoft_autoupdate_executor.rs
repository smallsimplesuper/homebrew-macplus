use std::path::Path;
use std::process::Command;

use crate::models::UpdateResult;
use crate::updaters::microsoft_autoupdate::lookup_hardcoded_token;
use crate::utils::{AppError, AppResult};
use super::homebrew_executor::HomebrewExecutor;
use super::UpdateExecutor;

/// Path to the Microsoft AutoUpdate `msupdate` CLI binary.
const MSUPDATE_PATH: &str = "/Library/Application Support/Microsoft/MAU2.0/Microsoft AutoUpdate.app/Contents/MacOS/msupdate";

/// Maps bundle IDs to the `msupdate --apps` identifiers.
const MSUPDATE_APP_IDS: &[(&str, &str)] = &[
    ("com.microsoft.Word", "MSWD2019"),
    ("com.microsoft.Excel", "XCEL2019"),
    ("com.microsoft.Powerpoint", "PPT32019"),
    ("com.microsoft.Outlook", "OPIM2019"),
    ("com.microsoft.onenote.mac", "ONMC2019"),
    ("com.microsoft.teams2", "TEAMS21"),
    ("com.microsoft.teams", "TEAMS10"),
    ("com.microsoft.OneDrive", "ONDR18"),
    ("com.microsoft.edgemac", "EDGE01"),
    ("com.microsoft.VSCode", "VSCO01"),
];

pub struct MicrosoftAutoUpdateExecutor {
    cask_token: Option<String>,
    pre_version: Option<String>,
    display_name: String,
}

impl MicrosoftAutoUpdateExecutor {
    pub fn new(display_name: String) -> Self {
        Self {
            cask_token: None,
            pre_version: None,
            display_name,
        }
    }

    pub fn with_cask_token(mut self, token: Option<String>) -> Self {
        self.cask_token = token;
        self
    }

    pub fn with_pre_version(mut self, version: Option<String>) -> Self {
        self.pre_version = version;
        self
    }

    /// Resolve a cask token from the detail or the hardcoded mapping.
    fn resolve_cask_token(&self, bundle_id: &str) -> Option<String> {
        self.cask_token.clone().or_else(|| {
            lookup_hardcoded_token(bundle_id).map(|t| t.to_string())
        })
    }

    /// Look up the msupdate app ID for a given bundle ID.
    fn msupdate_app_id(bundle_id: &str) -> Option<&'static str> {
        MSUPDATE_APP_IDS
            .iter()
            .find(|(bid, _)| *bid == bundle_id)
            .map(|(_, app_id)| *app_id)
    }

    /// Check whether Microsoft AutoUpdate is installed.
    fn mau_installed() -> bool {
        Path::new(MSUPDATE_PATH).exists()
    }
}

impl UpdateExecutor for MicrosoftAutoUpdateExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        // === Tier 1: Try Homebrew ===
        if let Some(token) = self.resolve_cask_token(bundle_id) {
            on_progress(5, "Trying Homebrew update...", None);
            log::info!("Microsoft executor: Tier 1 — trying brew upgrade --cask {}", token);

            let result = HomebrewExecutor::new(token.clone())
                .with_pre_version(self.pre_version.clone())
                .execute(bundle_id, app_path, on_progress)
                .await;

            match &result {
                Ok(r) if r.success => {
                    log::info!("Microsoft executor: Tier 1 succeeded for {}", bundle_id);
                    return result;
                }
                Ok(r) => {
                    log::info!(
                        "Microsoft executor: Tier 1 failed for {} ({}), trying Tier 2",
                        bundle_id,
                        r.message.as_deref().unwrap_or("unknown error")
                    );
                }
                Err(e) => {
                    log::info!(
                        "Microsoft executor: Tier 1 error for {} ({}), trying Tier 2",
                        bundle_id, e
                    );
                }
            }
        } else {
            log::info!("Microsoft executor: no cask token for {}, skipping Tier 1", bundle_id);
        }

        // === Tier 2: Try msupdate CLI ===
        if Self::mau_installed() {
            if let Some(app_id) = Self::msupdate_app_id(bundle_id) {
                on_progress(30, "Trying Microsoft AutoUpdate CLI...", None);
                log::info!("Microsoft executor: Tier 2 — trying msupdate --install --apps {}", app_id);

                let output = Command::new(MSUPDATE_PATH)
                    .args(["--install", "--apps", app_id])
                    .output();

                match output {
                    Ok(o) if o.status.success() => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        log::info!("Microsoft executor: Tier 2 succeeded for {}: {}", bundle_id, stdout.trim());
                        on_progress(100, "Microsoft AutoUpdate completed", None);

                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: true,
                            message: Some(format!(
                                "Updated {} via Microsoft AutoUpdate",
                                self.display_name
                            )),
                            source_type: "microsoft_autoupdate".to_string(),
                            from_version: self.pre_version.clone(),
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        log::info!(
                            "Microsoft executor: Tier 2 failed for {} (exit {}): {}",
                            bundle_id,
                            o.status.code().unwrap_or(-1),
                            stderr.trim()
                        );
                    }
                    Err(e) => {
                        log::info!("Microsoft executor: Tier 2 error for {}: {}", bundle_id, e);
                    }
                }
            } else {
                log::info!("Microsoft executor: no msupdate app ID for {}, skipping Tier 2", bundle_id);
            }
        } else {
            log::info!("Microsoft executor: MAU not installed, skipping Tier 2");
        }

        // === Tier 3: Open Microsoft AutoUpdate app (or the app itself) ===
        on_progress(50, "Opening Microsoft AutoUpdate...", None);

        if Self::mau_installed() {
            log::info!("Microsoft executor: Tier 3 — opening MAU app");
            let output = Command::new("open")
                .arg("-b")
                .arg("com.microsoft.autoupdate2")
                .output();

            match output {
                Ok(o) if o.status.success() => {
                    on_progress(100, "Opened Microsoft AutoUpdate", None);
                    return Ok(UpdateResult {
                        bundle_id: bundle_id.to_string(),
                        success: true,
                        message: Some(format!(
                            "Opened Microsoft AutoUpdate \u{2014} apply the update for {}",
                            self.display_name
                        )),
                        source_type: "microsoft_autoupdate".to_string(),
                        from_version: self.pre_version.clone(),
                        to_version: None,
                        handled_relaunch: false,
                        delegated: true,
                    });
                }
                _ => {
                    log::info!("Microsoft executor: failed to open MAU, falling back to opening app");
                }
            }
        }

        // Last resort: open the app itself
        log::info!("Microsoft executor: Tier 3 fallback — opening app at {}", app_path);
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
                    "Opened {} \u{2014} check for updates within the app",
                    self.display_name
                )),
                source_type: "microsoft_autoupdate".to_string(),
                from_version: self.pre_version.clone(),
                to_version: None,
                handled_relaunch: false,
                delegated: true,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            on_progress(100, &format!("Failed to open app: {}", stderr), None);
            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(format!("Failed to open {}: {}", app_path, stderr)),
                source_type: "microsoft_autoupdate".to_string(),
                from_version: self.pre_version.clone(),
                to_version: None,
                handled_relaunch: false,
                delegated: true,
            })
        }
    }
}
