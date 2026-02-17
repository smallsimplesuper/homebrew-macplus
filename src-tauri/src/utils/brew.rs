use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::askpass;

static BREW_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Returns the absolute path to the `brew` binary, resolved once and cached.
///
/// Checks well-known locations first (works in GUI context where PATH is minimal),
/// then falls back to `which brew` for non-standard installs.
pub fn brew_path() -> Option<&'static PathBuf> {
    BREW_PATH
        .get_or_init(|| {
            // Apple Silicon
            let apple_silicon = PathBuf::from("/opt/homebrew/bin/brew");
            if apple_silicon.exists() {
                log::info!("Found brew at {}", apple_silicon.display());
                return Some(apple_silicon);
            }

            // Intel Mac
            let intel = PathBuf::from("/usr/local/bin/brew");
            if intel.exists() {
                log::info!("Found brew at {}", intel.display());
                return Some(intel);
            }

            // Fallback: try `which brew` (works when PATH is available, e.g. cargo tauri dev)
            if let Ok(output) = Command::new("/usr/bin/which").current_dir("/tmp").arg("brew").output() {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path_str.is_empty() {
                        let path = PathBuf::from(&path_str);
                        if path.exists() {
                            log::info!("Found brew via which: {}", path.display());
                            return Some(path);
                        }
                    }
                }
            }

            log::warn!("Homebrew not found on this system");
            None
        })
        .as_ref()
}

/// Create a `Command` pre-configured for Homebrew invocations.
///
/// Sets `current_dir("/tmp")` (so brew doesn't complain about cwd) and, when
/// the askpass helper is available, injects `SUDO_ASKPASS` so that any nested
/// `sudo` calls inside brew can prompt the user via a native macOS dialog
/// instead of requiring a TTY.
pub fn brew_command(brew: &Path) -> Command {
    let mut cmd = Command::new(brew);
    cmd.current_dir("/tmp");
    if let Some(ap) = askpass::askpass_path() {
        cmd.env("SUDO_ASKPASS", ap);
        cmd.env(
            "SUDO_PROMPT",
            "macPlus needs your password to install this update:",
        );
    }
    cmd
}
