use std::process::Command;

/// Check if a GUI app with the given bundle ID is currently running.
/// Uses `lsappinfo list` which is the most reliable method for GUI apps on macOS.
pub fn is_app_running(bundle_id: &str) -> bool {
    let output = match Command::new("lsappinfo")
        .current_dir("/tmp")
        .args(["list"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains(bundle_id)
}

/// Quit an app gracefully via AppleScript, falling back to pkill if needed.
/// Returns true if the app was successfully quit (or wasn't running).
pub fn quit_app_gracefully(app_name: &str, bundle_id: &str) -> bool {
    // Try graceful quit via AppleScript
    let _ = Command::new("osascript")
        .current_dir("/tmp")
        .args(["-e", &format!("tell application id \"{}\" to quit", bundle_id)])
        .output();

    // Wait for app to quit
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Check if still running
    if !is_app_running(bundle_id) {
        return true;
    }

    // Force kill as fallback
    let _ = Command::new("pkill")
        .current_dir("/tmp")
        .args(["-x", app_name])
        .output();

    std::thread::sleep(std::time::Duration::from_millis(500));

    !is_app_running(bundle_id)
}

/// Relaunch an app in the background (won't bring to front).
pub fn relaunch_app(app_path: &str) {
    let _ = Command::new("open")
        .current_dir("/tmp")
        .args(["-g", app_path])
        .output();
}
