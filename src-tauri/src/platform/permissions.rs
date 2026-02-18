use std::ffi::CString;
use std::path::Path;
use std::process::Command;

/// Three-state permission result for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    Denied,
    Unknown,
}

impl PermissionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_granted(&self) -> bool {
        matches!(self, Self::Granted)
    }
}

/// Check if the app has Full Disk Access by testing read access to a protected path.
pub fn has_full_disk_access() -> bool {
    Path::new("/Library/Application Support/com.apple.TCC/TCC.db").exists()
        && std::fs::metadata("/Library/Application Support/com.apple.TCC/TCC.db").is_ok()
}

/// Passively check Automation (Apple Events) permission by reading the user TCC database.
/// Does NOT trigger any macOS permission dialog.
pub fn check_automation_passive() -> PermissionState {
    let db_path = match dirs::home_dir() {
        Some(h) => h.join("Library/Application Support/com.apple.TCC/TCC.db"),
        None => return PermissionState::Unknown,
    };

    if !db_path.exists() {
        return PermissionState::Unknown;
    }

    // Open read-only with SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = match rusqlite::Connection::open_with_flags(&db_path, flags) {
        Ok(c) => c,
        Err(_) => return PermissionState::Unknown,
    };

    // auth_value: 2 = allowed, 0 = denied
    let result = conn.query_row(
        "SELECT auth_value FROM access WHERE service = 'kTCCServiceAppleEvents' \
         AND client = 'com.macplus.app' \
         AND indirect_object_identifier = 'com.apple.systemevents'",
        [],
        |row| row.get::<_, i64>(0),
    );

    match result {
        Ok(2) => PermissionState::Granted,
        Ok(_) => PermissionState::Denied,
        Err(_) => PermissionState::Unknown,
    }
}

/// Intentionally trigger the macOS Automation permission dialog by running an osascript probe.
/// Call ONLY when the user clicks "Enable" for Automation.
///
/// Uses spawn + poll with a 3-second deadline instead of blocking `output()`,
/// which can hang indefinitely on macOS 26 (Tahoe). If the process doesn't
/// complete in time, it is killed to prevent thread leaks.
pub fn trigger_automation_permission() -> bool {
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

/// Check if the app has notification permission via macOS notification center prefs.
pub fn has_notification_permission(bundle_id: &str) -> bool {
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
    false
}

/// Check if the app has App Management permission using POSIX access() check.
/// Does NOT trigger any macOS permission dialog — just tests write access to /Applications.
pub fn has_app_management() -> bool {
    let path = CString::new("/Applications").unwrap();
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}
