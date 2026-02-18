use std::path::PathBuf;
use std::sync::RwLock;

static ASKPASS_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Resolve the bundled `macplus-askpass` script, ensure it is executable, and
/// cache its path.  Can be called multiple times — always re-checks the path.
pub fn init_askpass_path(resource_dir: PathBuf) {
    let script = resource_dir.join("macplus-askpass");
    if !script.exists() {
        log::warn!("askpass helper not found at {}", script.display());
        return;
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
    if let Ok(mut guard) = ASKPASS_PATH.write() {
        *guard = Some(script);
    }
}

/// Returns the cached path to the askpass helper, if available.
pub fn askpass_path() -> Option<PathBuf> {
    ASKPASS_PATH.read().ok().and_then(|g| g.clone())
}

/// Returns `true` when the helper exists and is executable.
pub fn is_askpass_installed() -> bool {
    askpass_path().map_or(false, |p| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(&p)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            p.exists()
        }
    })
}
