use std::path::PathBuf;
use std::sync::OnceLock;

static ASKPASS_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Resolve the bundled `macplus-askpass` script, ensure it is executable, and
/// cache its path.  Called once during app startup.
pub fn init_askpass_path(resource_dir: PathBuf) {
    ASKPASS_PATH.get_or_init(|| {
        let script = resource_dir.join("macplus-askpass");
        if !script.exists() {
            log::warn!("askpass helper not found at {}", script.display());
            return None;
        }

        // Ensure the script is executable (it may lose the bit when bundled).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&script) {
                let mut perms = meta.permissions();
                let mode = perms.mode();
                if mode & 0o111 == 0 {
                    perms.set_mode(mode | 0o755);
                    let _ = std::fs::set_permissions(&script, perms);
                }
            }
        }

        log::info!("askpass helper ready at {}", script.display());
        Some(script)
    });
}

/// Returns the cached path to the askpass helper, if available.
pub fn askpass_path() -> Option<&'static PathBuf> {
    ASKPASS_PATH.get().and_then(|p| p.as_ref())
}

/// Returns `true` when the helper exists and is executable.
pub fn is_askpass_installed() -> bool {
    askpass_path().map_or(false, |p| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(p)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            p.exists()
        }
    })
}
