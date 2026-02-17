use std::path::Path;
use std::process::Command;

use regex::Regex;

use crate::detection::bundle_reader;
use crate::models::UpdateResult;
use crate::utils::brew::{brew_command, brew_path};
use crate::utils::{AppError, AppResult};
use super::UpdateExecutor;

pub struct HomebrewExecutor {
    cask_token: String,
    pre_version: Option<String>,
}

impl HomebrewExecutor {
    pub fn new(cask_token: String) -> Self {
        Self { cask_token, pre_version: None }
    }

    pub fn with_pre_version(mut self, version: Option<String>) -> Self {
        self.pre_version = version;
        self
    }
}

/// Extract a .pkg path from brew error output (e.g. `/opt/homebrew/Caskroom/…/foo.pkg`).
fn extract_pkg_path(error_msg: &str) -> Option<String> {
    let re = Regex::new(r#"(/opt/homebrew/Caskroom/[^\s'"]+\.pkg|/usr/local/Caskroom/[^\s'"]+\.pkg)"#)
        .ok()?;
    re.find(error_msg).map(|m| m.as_str().to_string())
}

impl HomebrewExecutor {
    /// Check whether the cask is already installed via Homebrew.
    fn is_cask_installed(&self, brew: &Path) -> bool {
        brew_command(brew)
            .args(["list", "--cask", &self.cask_token])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl UpdateExecutor for HomebrewExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        _app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        let brew = brew_path()
            .ok_or_else(|| AppError::CommandFailed("Homebrew not found".to_string()))?;

        // Capture pre-install version from the app bundle
        let pre_version = self.pre_version.clone().or_else(|| {
            bundle_reader::read_bundle(Path::new(_app_path))
                .and_then(|b| b.installed_version)
        });

        // If the cask is already installed via Homebrew, upgrade it.
        // Otherwise, install it (this handles apps installed directly outside of brew).
        on_progress(5, "Checking cask status...", None);

        let (action, action_past) = if self.is_cask_installed(brew) {
            ("upgrade", "upgraded")
        } else {
            ("install", "installed")
        };

        on_progress(10, &format!("Preparing to {} cask...", action), None);

        let mut args = vec![action, "--cask", &self.cask_token];
        // When installing (not upgrading), force is needed to overwrite
        // an existing app bundle that wasn't installed via Homebrew.
        if action == "install" {
            args.push("--force");
        }

        on_progress(20, &format!("Running brew {}...", action), None);

        let output = brew_command(brew)
            .args(&args)
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to run brew: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            on_progress(50, "Brew command completed", None);

            // Re-read bundle to check if version actually changed
            let new_version = bundle_reader::read_bundle(Path::new(_app_path))
                .and_then(|b| b.installed_version);

            let actually_changed = match (&pre_version, &new_version) {
                (Some(old), Some(new)) => old != new,
                _ => true, // If we can't compare, trust the exit code
            };

            if !actually_changed {
                let msg = format!(
                    "Homebrew reported success but {} is still at version {}. \
                     Try running 'brew upgrade --cask {}' manually.",
                    self.cask_token,
                    pre_version.as_deref().unwrap_or("unknown"),
                    self.cask_token
                );
                on_progress(100, &msg, None);
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: false,
                    message: Some(msg),
                    source_type: "homebrew_cask".to_string(),
                    from_version: pre_version,
                    to_version: new_version,
                    handled_relaunch: false,
                    delegated: false,
                });
            }

            // Best-effort cleanup — ignore errors
            on_progress(90, "Running cleanup...", None);
            let _ = brew_command(brew)
                .args(["cleanup", &self.cask_token])
                .output();

            on_progress(100, &format!("Homebrew {} completed successfully", action), None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: true,
                message: Some(format!("Successfully {} cask '{}'", action_past, self.cask_token)),
                source_type: "homebrew_cask".to_string(),
                from_version: pre_version,
                to_version: new_version,
                handled_relaunch: false,
                delegated: false,
            })
        } else {
            let error_msg = if stderr.is_empty() { &stdout } else { &stderr };

            // Detect permission/elevation errors and retry with administrator privileges
            let needs_elevation =
                (error_msg.contains("sudo") && error_msg.contains("password"))
                || error_msg.contains("terminal is required")
                || error_msg.contains("tty")
                || error_msg.contains("Operation not permitted")
                || error_msg.contains("Permission denied")
                || error_msg.contains("cannot access parent directories");

            if needs_elevation {
                on_progress(30, "Requesting administrator privileges...", None);

                // Check if the error contains a .pkg path — these need direct
                // installation because `sudo -u $USER brew …` as root will hit
                // a nested-sudo wall when brew tries to run `sudo installer`.
                if let Some(pkg_path) = extract_pkg_path(error_msg) {
                    on_progress(35, "Installing package directly...", None);

                    let pkg_args: Vec<&str> = vec!["-pkg", &pkg_path, "-target", "/"];
                    match crate::utils::sudo_session::run_elevated("/usr/sbin/installer", &pkg_args) {
                        Ok(pkg_output) if pkg_output.status.success() => {
                            on_progress(60, "Package installed, finalizing with brew...", None);

                            // Re-run brew so it reconciles its internal state
                            let _ = brew_command(brew)
                                .args(&args)
                                .output();

                            on_progress(70, "Verifying installation...", None);

                            let new_version = bundle_reader::read_bundle(Path::new(_app_path))
                                .and_then(|b| b.installed_version);

                            let actually_changed = match (&pre_version, &new_version) {
                                (Some(old), Some(new)) => old != new,
                                _ => true,
                            };

                            if !actually_changed {
                                let msg = format!(
                                    "Package installed but {} is still at version {}. \
                                     Run 'brew upgrade --cask {}' in Terminal.app to complete this update.",
                                    self.cask_token,
                                    pre_version.as_deref().unwrap_or("unknown"),
                                    self.cask_token
                                );
                                on_progress(100, &msg, None);
                                return Ok(UpdateResult {
                                    bundle_id: bundle_id.to_string(),
                                    success: false,
                                    message: Some(msg),
                                    source_type: "homebrew_cask".to_string(),
                                    from_version: pre_version,
                                    to_version: new_version,
                                    handled_relaunch: false,
                                    delegated: false,
                                });
                            }

                            on_progress(90, "Running cleanup...", None);
                            let _ = brew_command(brew)
                                .args(["cleanup", &self.cask_token])
                                .output();

                            on_progress(100, &format!("Homebrew {} completed successfully", action), None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: true,
                                message: Some(format!(
                                    "Successfully {} cask '{}' (pkg installed with admin privileges)",
                                    action_past, self.cask_token
                                )),
                                source_type: "homebrew_cask".to_string(),
                                from_version: pre_version,
                                to_version: new_version,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }
                        Ok(_) | Err(crate::utils::sudo_session::ElevatedError::CommandFailed(_))
                        | Err(crate::utils::sudo_session::ElevatedError::IoError(_)) => {
                            let msg = format!(
                                "Package installation failed. \
                                 Run 'brew upgrade --cask {}' in Terminal.app to complete this update.",
                                self.cask_token
                            );
                            on_progress(100, &msg, None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: false,
                                message: Some(msg),
                                source_type: "homebrew_cask".to_string(),
                                from_version: pre_version,
                                to_version: None,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }
                        Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                            let msg = "Update cancelled \u{2014} administrator approval is required for this cask".to_string();
                            on_progress(100, &msg, None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: false,
                                message: Some(msg),
                                source_type: "homebrew_cask".to_string(),
                                from_version: pre_version,
                                to_version: None,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }
                    }
                }

                // Retry with SUDO_ASKPASS + sudo -A — if the askpass helper is
                // configured this will show a native password dialog and succeed
                // without needing osascript elevation.
                if crate::utils::askpass::askpass_path().is_some() {
                    on_progress(30, "Retrying with askpass helper...", None);

                    let mut retry_args = vec!["-A", brew.to_str().unwrap_or("brew")];
                    retry_args.extend(args.iter().copied());

                    let mut retry_cmd = Command::new("sudo");
                    retry_cmd.current_dir("/tmp");
                    if let Some(ap) = crate::utils::askpass::askpass_path() {
                        retry_cmd.env("SUDO_ASKPASS", ap);
                    }
                    retry_cmd.args(&retry_args);

                    if let Ok(retry_out) = retry_cmd.output() {
                        if retry_out.status.success() {
                            on_progress(60, "Brew command completed", None);
                            let new_version = bundle_reader::read_bundle(Path::new(_app_path))
                                .and_then(|b| b.installed_version);
                            let actually_changed = match (&pre_version, &new_version) {
                                (Some(old), Some(new)) => old != new,
                                _ => true,
                            };
                            if actually_changed {
                                on_progress(90, "Running cleanup...", None);
                                let _ = brew_command(brew)
                                    .args(["cleanup", &self.cask_token])
                                    .output();
                                on_progress(100, &format!("Homebrew {} completed successfully", action), None);
                                return Ok(UpdateResult {
                                    bundle_id: bundle_id.to_string(),
                                    success: true,
                                    message: Some(format!(
                                        "Successfully {} cask '{}' (with askpass helper)",
                                        action_past, self.cask_token
                                    )),
                                    source_type: "homebrew_cask".to_string(),
                                    from_version: pre_version,
                                    to_version: new_version,
                                    handled_relaunch: false,
                                    delegated: false,
                                });
                            }
                        }
                        // If askpass retry failed or version didn't change, fall through
                        // to the osascript approach below.
                    }
                }

                // No .pkg path found — use the general elevated approach
                // (sudo -u $USER brew …) which works for non-pkg casks.
                let current_user = std::env::var("USER").unwrap_or_else(|_| "".to_string());
                let brew_cmd = if current_user.is_empty() {
                    format!(
                        "cd /tmp && {} {} --cask {}{}",
                        brew.display(),
                        action,
                        self.cask_token,
                        if action == "install" { " --force" } else { "" }
                    )
                } else {
                    format!(
                        "cd /tmp && sudo -u {} {} {} --cask {}{}",
                        current_user,
                        brew.display(),
                        action,
                        self.cask_token,
                        if action == "install" { " --force" } else { "" }
                    )
                };

                match crate::utils::sudo_session::run_elevated_shell(&brew_cmd) {
                    Ok(osa_output) if osa_output.status.success() => {
                        on_progress(60, "Brew command completed", None);

                        let new_version = bundle_reader::read_bundle(Path::new(_app_path))
                            .and_then(|b| b.installed_version);

                        let actually_changed = match (&pre_version, &new_version) {
                            (Some(old), Some(new)) => old != new,
                            _ => true,
                        };

                        if !actually_changed {
                            let msg = format!(
                                "Homebrew reported success but {} is still at version {}. \
                                 Try running 'brew upgrade --cask {}' manually.",
                                self.cask_token,
                                pre_version.as_deref().unwrap_or("unknown"),
                                self.cask_token
                            );
                            on_progress(100, &msg, None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: false,
                                message: Some(msg),
                                source_type: "homebrew_cask".to_string(),
                                from_version: pre_version,
                                to_version: new_version,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }

                        on_progress(90, "Running cleanup...", None);
                        let _ = brew_command(brew)
                            .args(["cleanup", &self.cask_token])
                            .output();
                        on_progress(100, &format!("Homebrew {} completed successfully", action), None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: true,
                            message: Some(format!(
                                "Successfully {} cask '{}' (with admin privileges)",
                                action_past, self.cask_token
                            )),
                            source_type: "homebrew_cask".to_string(),
                            from_version: pre_version,
                            to_version: new_version,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                        let msg = "Update cancelled \u{2014} administrator approval is required for this cask".to_string();
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: "homebrew_cask".to_string(),
                            from_version: pre_version,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Ok(osa_output) => {
                        let osa_stderr = String::from_utf8_lossy(&osa_output.stderr).to_string();
                        let msg = format!("Homebrew {} failed (elevated): {}", action, osa_stderr);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: "homebrew_cask".to_string(),
                            from_version: pre_version,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(e) => {
                        let msg = format!("Homebrew {} failed: could not request admin privileges: {}", action, e);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: "homebrew_cask".to_string(),
                            from_version: pre_version,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                }
            }

            // Check for App Management permission issue specifically
            if error_msg.contains("Operation not permitted") || error_msg.contains("cannot access parent directories") {
                let msg = "macOS blocked Homebrew from modifying /Applications. \
                     Grant macPlus 'App Management' permission in System Settings > \
                     Privacy & Security > App Management, then try again.".to_string();
                on_progress(100, &msg, None);
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: false,
                    message: Some(msg),
                    source_type: "homebrew_cask".to_string(),
                    from_version: pre_version,
                    to_version: None,
                    handled_relaunch: false,
                    delegated: false,
                });
            }

            // Non-sudo error — return as-is
            let msg = format!("Homebrew {} failed: {}", action, error_msg);
            on_progress(100, &msg, None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(format!(
                    "Failed to {} cask '{}': {}",
                    action, self.cask_token, error_msg
                )),
                source_type: "homebrew_cask".to_string(),
                from_version: pre_version,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            })
        }
    }
}
