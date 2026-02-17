use std::process::Command;

use regex::Regex;

use crate::models::UpdateResult;
use crate::utils::brew::{brew_command, brew_path};
use crate::utils::{is_xcode_clt_installed, AppError, AppResult};
use super::UpdateExecutor;

pub struct HomebrewFormulaExecutor {
    formula_name: String,
    pre_version: Option<String>,
}

impl HomebrewFormulaExecutor {
    pub fn new(formula_name: String) -> Self {
        Self { formula_name, pre_version: None }
    }

    pub fn with_pre_version(mut self, version: Option<String>) -> Self {
        self.pre_version = version;
        self
    }

    /// Get the currently installed version of a formula via `brew info --json=v2`.
    fn get_formula_version(brew: &std::path::Path, formula: &str) -> Option<String> {
        let output = brew_command(brew)
            .args(["info", "--json=v2", formula])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        json["formulae"]
            .as_array()?
            .first()?
            .get("installed")?
            .as_array()?
            .last()?
            .get("version")?
            .as_str()
            .map(String::from)
    }
}

/// Extract a .pkg path from brew error output (e.g. `/opt/homebrew/Caskroom/…/foo.pkg`).
fn extract_pkg_path(error_msg: &str) -> Option<String> {
    let re = Regex::new(r#"(/opt/homebrew/Cellar/[^\s'"]+\.pkg|/usr/local/Cellar/[^\s'"]+\.pkg)"#)
        .ok()?;
    re.find(error_msg).map(|m| m.as_str().to_string())
}

impl UpdateExecutor for HomebrewFormulaExecutor {
    async fn execute(
        &self,
        bundle_id: &str,
        _app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult> {
        let brew = brew_path()
            .ok_or_else(|| AppError::CommandFailed("Homebrew not found".to_string()))?;

        // Pre-flight: ensure Xcode Command Line Tools are installed
        if !is_xcode_clt_installed() {
            let msg = "Xcode Command Line Tools required. Install with: xcode-select --install";
            on_progress(100, &msg, None);
            return Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(msg.to_string()),
                source_type: "homebrew_formula".to_string(),
                from_version: None,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            });
        }

        // Capture pre-install version
        let pre_version = self.pre_version.clone().or_else(|| {
            Self::get_formula_version(&brew, &self.formula_name)
        });

        on_progress(5, "Checking formula status...", None);
        on_progress(10, &format!("Preparing to upgrade {}...", self.formula_name), None);
        on_progress(20, &format!("Running brew upgrade {}...", self.formula_name), None);

        let output = brew_command(&brew)
            .args(["upgrade", &self.formula_name])
            .output()
            .map_err(|e| AppError::CommandFailed(format!("Failed to run brew: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            on_progress(50, "Brew command completed", None);

            let new_version = Self::get_formula_version(&brew, &self.formula_name);

            let actually_changed = match (&pre_version, &new_version) {
                (Some(old), Some(new)) => old != new,
                _ => true,
            };

            if !actually_changed {
                let msg = format!(
                    "Homebrew reported success but {} is still at version {}. \
                     Try running 'brew upgrade {}' manually.",
                    self.formula_name,
                    pre_version.as_deref().unwrap_or("unknown"),
                    self.formula_name
                );
                on_progress(100, &msg, None);
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: false,
                    message: Some(msg),
                    source_type: "homebrew_formula".to_string(),
                    from_version: pre_version,
                    to_version: new_version,
                    handled_relaunch: false,
                    delegated: false,
                });
            }

            on_progress(90, "Running cleanup...", None);
            let _ = brew_command(&brew)
                .args(["cleanup", &self.formula_name])
                .output();

            on_progress(100, &format!("{} upgraded successfully", self.formula_name), None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: true,
                message: Some(format!("Successfully upgraded formula '{}'", self.formula_name)),
                source_type: "homebrew_formula".to_string(),
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

                // Check if the error contains a .pkg path — direct install bypasses
                // nested-sudo issues.
                if let Some(pkg_path) = extract_pkg_path(error_msg) {
                    on_progress(35, "Installing package directly...", None);

                    let pkg_args: Vec<&str> = vec!["-pkg", &pkg_path, "-target", "/"];
                    match crate::utils::sudo_session::run_elevated("/usr/sbin/installer", &pkg_args) {
                        Ok(pkg_output) if pkg_output.status.success() => {
                            on_progress(60, "Package installed, finalizing with brew...", None);

                            // Re-run brew so it reconciles its internal state
                            let _ = brew_command(&brew)
                                .args(["upgrade", &self.formula_name])
                                .output();

                            on_progress(70, "Verifying installation...", None);

                            let new_version = Self::get_formula_version(&brew, &self.formula_name);

                            let actually_changed = match (&pre_version, &new_version) {
                                (Some(old), Some(new)) => old != new,
                                _ => true,
                            };

                            if !actually_changed {
                                let msg = format!(
                                    "Package installed but {} is still at version {}. \
                                     Run 'brew upgrade {}' in Terminal.app to complete this update.",
                                    self.formula_name,
                                    pre_version.as_deref().unwrap_or("unknown"),
                                    self.formula_name
                                );
                                on_progress(100, &msg, None);
                                return Ok(UpdateResult {
                                    bundle_id: bundle_id.to_string(),
                                    success: false,
                                    message: Some(msg),
                                    source_type: "homebrew_formula".to_string(),
                                    from_version: pre_version,
                                    to_version: new_version,
                                    handled_relaunch: false,
                                    delegated: false,
                                });
                            }

                            on_progress(90, "Running cleanup...", None);
                            let _ = brew_command(&brew)
                                .args(["cleanup", &self.formula_name])
                                .output();

                            on_progress(100, &format!("{} upgraded successfully", self.formula_name), None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: true,
                                message: Some(format!(
                                    "Successfully upgraded formula '{}' (pkg installed with admin privileges)",
                                    self.formula_name
                                )),
                                source_type: "homebrew_formula".to_string(),
                                from_version: pre_version,
                                to_version: new_version,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }
                        Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                            let msg = "Upgrade cancelled \u{2014} administrator approval is required".to_string();
                            on_progress(100, &msg, None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: false,
                                message: Some(msg),
                                source_type: "homebrew_formula".to_string(),
                                from_version: pre_version,
                                to_version: None,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }
                        Ok(_) | Err(_) => {
                            let msg = format!(
                                "Package installation failed. \
                                 Run 'brew upgrade {}' in Terminal.app to complete this update.",
                                self.formula_name
                            );
                            on_progress(100, &msg, None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: false,
                                message: Some(msg),
                                source_type: "homebrew_formula".to_string(),
                                from_version: pre_version,
                                to_version: None,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }
                    }
                }

                // Retry with SUDO_ASKPASS + sudo -A
                if crate::utils::askpass::askpass_path().is_some() {
                    on_progress(30, "Retrying with askpass helper...", None);

                    let mut retry_cmd = Command::new("sudo");
                    retry_cmd.current_dir("/tmp");
                    if let Some(ap) = crate::utils::askpass::askpass_path() {
                        retry_cmd.env("SUDO_ASKPASS", ap);
                    }
                    retry_cmd.args(["-A", brew.to_str().unwrap_or("brew"), "upgrade", &self.formula_name]);

                    if let Ok(retry_out) = retry_cmd.output() {
                        if retry_out.status.success() {
                            on_progress(60, "Brew command completed", None);
                            let new_version = Self::get_formula_version(&brew, &self.formula_name);
                            let actually_changed = match (&pre_version, &new_version) {
                                (Some(old), Some(new)) => old != new,
                                _ => true,
                            };
                            if actually_changed {
                                on_progress(90, "Running cleanup...", None);
                                let _ = brew_command(&brew)
                                    .args(["cleanup", &self.formula_name])
                                    .output();
                                on_progress(100, &format!("{} upgraded successfully", self.formula_name), None);
                                return Ok(UpdateResult {
                                    bundle_id: bundle_id.to_string(),
                                    success: true,
                                    message: Some(format!(
                                        "Successfully upgraded formula '{}' (with askpass helper)",
                                        self.formula_name
                                    )),
                                    source_type: "homebrew_formula".to_string(),
                                    from_version: pre_version,
                                    to_version: new_version,
                                    handled_relaunch: false,
                                    delegated: false,
                                });
                            }
                        }
                    }
                }

                // No .pkg path found — use the general elevated approach
                let current_user = std::env::var("USER").unwrap_or_else(|_| "".to_string());
                let brew_cmd = if current_user.is_empty() {
                    format!("cd /tmp && {} upgrade {}", brew.display(), self.formula_name)
                } else {
                    format!(
                        "cd /tmp && sudo -u {} {} upgrade {}",
                        current_user,
                        brew.display(),
                        self.formula_name
                    )
                };

                match crate::utils::sudo_session::run_elevated_shell(&brew_cmd) {
                    Ok(osa_output) if osa_output.status.success() => {
                        on_progress(60, "Brew command completed", None);

                        let new_version = Self::get_formula_version(&brew, &self.formula_name);

                        let actually_changed = match (&pre_version, &new_version) {
                            (Some(old), Some(new)) => old != new,
                            _ => true,
                        };

                        if !actually_changed {
                            let msg = format!(
                                "Homebrew reported success but {} is still at version {}. \
                                 Try running 'brew upgrade {}' manually.",
                                self.formula_name,
                                pre_version.as_deref().unwrap_or("unknown"),
                                self.formula_name
                            );
                            on_progress(100, &msg, None);
                            return Ok(UpdateResult {
                                bundle_id: bundle_id.to_string(),
                                success: false,
                                message: Some(msg),
                                source_type: "homebrew_formula".to_string(),
                                from_version: pre_version,
                                to_version: new_version,
                                handled_relaunch: false,
                                delegated: false,
                            });
                        }

                        on_progress(90, "Running cleanup...", None);
                        let _ = brew_command(&brew)
                            .args(["cleanup", &self.formula_name])
                            .output();

                        on_progress(100, &format!("{} upgraded successfully", self.formula_name), None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: true,
                            message: Some(format!(
                                "Successfully upgraded formula '{}' (with admin privileges)",
                                self.formula_name
                            )),
                            source_type: "homebrew_formula".to_string(),
                            from_version: pre_version,
                            to_version: new_version,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                        let msg = format!(
                            "Upgrade cancelled \u{2014} administrator approval required for {}",
                            self.formula_name
                        );
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: "homebrew_formula".to_string(),
                            from_version: pre_version,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Ok(osa_output) => {
                        let osa_stderr = String::from_utf8_lossy(&osa_output.stderr).to_string();
                        let msg = format!("Homebrew upgrade failed (elevated): {}", osa_stderr);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: "homebrew_formula".to_string(),
                            from_version: pre_version,
                            to_version: None,
                            handled_relaunch: false,
                            delegated: false,
                        });
                    }
                    Err(e) => {
                        let msg = format!("Homebrew upgrade failed: could not request admin privileges: {}", e);
                        on_progress(100, &msg, None);
                        return Ok(UpdateResult {
                            bundle_id: bundle_id.to_string(),
                            success: false,
                            message: Some(msg),
                            source_type: "homebrew_formula".to_string(),
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
                let msg = "macOS blocked Homebrew from modifying system files. \
                     Grant macPlus 'App Management' permission in System Settings > \
                     Privacy & Security > App Management, then try again.".to_string();
                on_progress(100, &msg, None);
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: false,
                    message: Some(msg),
                    source_type: "homebrew_formula".to_string(),
                    from_version: pre_version,
                    to_version: None,
                    handled_relaunch: false,
                    delegated: false,
                });
            }

            let msg = format!("Homebrew upgrade failed: {}", error_msg);
            on_progress(100, &msg, None);

            Ok(UpdateResult {
                bundle_id: bundle_id.to_string(),
                success: false,
                message: Some(format!(
                    "Failed to upgrade formula '{}': {}",
                    self.formula_name, error_msg
                )),
                source_type: "homebrew_formula".to_string(),
                from_version: pre_version,
                to_version: None,
                handled_relaunch: false,
                delegated: false,
            })
        }
    }
}
