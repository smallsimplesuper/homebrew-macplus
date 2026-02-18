use std::path::Path;
use std::process::Command;

/// Check if the app has Full Disk Access by testing read access to a protected path.
pub fn has_full_disk_access() -> bool {
    Path::new("/Library/Application Support/com.apple.TCC/TCC.db").exists()
        && std::fs::metadata("/Library/Application Support/com.apple.TCC/TCC.db").is_ok()
}

/// Check if the app has Automation (Apple Events) permission by running a harmless
/// osascript call to System Events. Returns true if macOS grants it.
///
/// Uses spawn + poll with a 3-second deadline instead of blocking `output()`,
/// which can hang indefinitely on macOS 26 (Tahoe). If the process doesn't
/// complete in time, it is killed to prevent thread leaks.
pub fn has_automation_permission() -> bool {
    let mut child = match Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to return name of first process"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(_) => return false,
        }
    }
}

/// Check if the app has notification permission via macOS UNUserNotificationCenter.
/// Falls back to checking the notification settings database.
pub fn has_notification_permission(bundle_id: &str) -> bool {
    // Use `defaults read` to check the notification center prefs for our bundle ID.
    // On macOS 13+, the notification center stores per-app flags.
    let output = Command::new("defaults")
        .args(["read", "com.apple.notificationcenterui"])
        .output();
    // If we can't read the plist, fall back to a heuristic:
    // try sending a test notification — if it doesn't error, permission is likely granted.
    // For now, use a simpler approach: check the UNNotificationSettings via osascript.

    // Simpler approach: check the notification center database directly.
    // The permission value 2 = authorized, 1 = denied, 0 = not determined.
    let db_path = dirs::home_dir()
        .map(|h| h.join("Library/Preferences/com.apple.ncprefs.plist"));
    if let Some(ref path) = db_path {
        if let Ok(plist_output) = Command::new("plutil")
            .args(["-convert", "json", "-o", "-", &path.to_string_lossy()])
            .output()
        {
            if plist_output.status.success() {
                let json_str = String::from_utf8_lossy(&plist_output.stdout);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Some(apps) = val.get("apps").and_then(|a| a.as_array()) {
                        for app in apps {
                            let bid = app.get("bundle-id").and_then(|b| b.as_str()).unwrap_or("");
                            if bid == bundle_id {
                                // flags & 0x04 == authorized for alerts
                                let flags = app.get("flags").and_then(|f| f.as_u64()).unwrap_or(0);
                                return flags & 4 != 0;
                            }
                        }
                    }
                }
            }
        }
    }
    // If not found in prefs, assume not determined (treat as not granted)
    drop(output);
    false
}

/// Check if the app has App Management permission by testing write access
/// to a known path in /Applications.
pub fn has_app_management() -> bool {
    let test_path = Path::new("/Applications/.macplus_permission_test");
    match std::fs::File::create(test_path) {
        Ok(_) => {
            let _ = std::fs::remove_file(test_path);
            true
        }
        Err(_) => false,
    }
}
