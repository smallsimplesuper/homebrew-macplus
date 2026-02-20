use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Cache: once we know Automation is granted, remember it across TCC re-reads.
static AUTOMATION_KNOWN_GRANTED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Persistent automation cache (survives app restarts)
// ---------------------------------------------------------------------------

fn automation_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("com.macplus.app").join("automation_granted"))
}

fn write_automation_cache() {
    if let Some(path) = automation_cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, b"1");
        log::debug!("automation: wrote persistent cache at {:?}", path);
    }
}

fn clear_automation_cache() {
    if let Some(path) = automation_cache_path() {
        let _ = std::fs::remove_file(&path);
        log::debug!("automation: cleared persistent cache at {:?}", path);
    }
}

/// Check if the app has Full Disk Access by spawning a subprocess that reads a
/// TCC-protected file. A child process gets a fresh TCC evaluation, bypassing
/// any per-process access caching in the parent.
pub fn has_full_disk_access() -> bool {
    Command::new("/bin/cat")
        .arg("/Library/Application Support/com.apple.TCC/TCC.db")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Passively check Automation (Apple Events) permission by reading the user TCC database.
/// Does NOT trigger any macOS permission dialog.
pub fn check_automation_passive() -> PermissionState {
    let db_path = match dirs::home_dir() {
        Some(h) => h.join("Library/Application Support/com.apple.TCC/TCC.db"),
        None => {
            log::debug!("automation: no home dir, falling back to in-memory cache");
            return if AUTOMATION_KNOWN_GRANTED.load(Ordering::Relaxed) {
                PermissionState::Granted
            } else {
                PermissionState::Unknown
            };
        }
    };

    if !db_path.exists() {
        log::debug!("automation: TCC db not found at {:?}", db_path);
        return if AUTOMATION_KNOWN_GRANTED.load(Ordering::Relaxed) {
            PermissionState::Granted
        } else {
            PermissionState::Unknown
        };
    }

    // Open read-only with SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = match rusqlite::Connection::open_with_flags(&db_path, flags) {
        Ok(c) => c,
        Err(e) => {
            log::debug!("automation: failed to open TCC db: {e}");
            return if AUTOMATION_KNOWN_GRANTED.load(Ordering::Relaxed) {
                PermissionState::Granted
            } else {
                PermissionState::Unknown
            };
        }
    };

    // Try exact match first: client = 'com.macplus.app' targeting System Events
    let result = conn.query_row(
        "SELECT auth_value FROM access WHERE service = 'kTCCServiceAppleEvents' \
         AND client = 'com.macplus.app' \
         AND indirect_object_identifier = 'com.apple.systemevents'",
        [],
        |row| row.get::<_, i64>(0),
    );

    match result {
        Ok(2) => {
            AUTOMATION_KNOWN_GRANTED.store(true, Ordering::Relaxed);
            log::debug!("automation: granted via TCC exact match");
            return PermissionState::Granted;
        }
        Ok(val) => {
            log::debug!("automation: denied via TCC exact match (auth_value={val})");
            return PermissionState::Denied;
        }
        Err(_) => {}
    }

    // Fallback: broader query — any Apple Events grant for our app targeting any Apple app
    let broad_result = conn.query_row(
        "SELECT auth_value FROM access WHERE service = 'kTCCServiceAppleEvents' \
         AND client = 'com.macplus.app' \
         AND indirect_object_identifier LIKE 'com.apple.%' \
         LIMIT 1",
        [],
        |row| row.get::<_, i64>(0),
    );

    match broad_result {
        Ok(2) => {
            AUTOMATION_KNOWN_GRANTED.store(true, Ordering::Relaxed);
            log::debug!("automation: granted via TCC broad match");
            return PermissionState::Granted;
        }
        Ok(val) => {
            log::debug!("automation: denied via TCC broad match (auth_value={val})");
            return PermissionState::Denied;
        }
        Err(_) => {
            log::debug!("automation: no TCC rows found for our bundle ID");
        }
    }

    // TCC returned nothing — check persistent file cache from a prior session
    if !AUTOMATION_KNOWN_GRANTED.load(Ordering::Relaxed) {
        if let Some(cache_path) = automation_cache_path() {
            if cache_path.exists() {
                log::debug!("automation: persistent cache exists, running probe to verify");
                match quick_automation_probe(1500) {
                    Some(true) => {
                        AUTOMATION_KNOWN_GRANTED.store(true, Ordering::Relaxed);
                        log::debug!("automation: probe confirmed grant (cache valid)");
                        return PermissionState::Granted;
                    }
                    Some(false) => {
                        clear_automation_cache();
                        log::debug!("automation: probe denied, cleared stale cache");
                        return PermissionState::Denied;
                    }
                    None => {
                        // Probe timed out (cold start) — trust the persistent cache
                        AUTOMATION_KNOWN_GRANTED.store(true, Ordering::Relaxed);
                        log::debug!("automation: probe timed out, trusting persistent cache");
                        return PermissionState::Granted;
                    }
                }
            }
        }
    }

    // No cache — try a quick osascript probe
    if !AUTOMATION_KNOWN_GRANTED.load(Ordering::Relaxed) {
        if quick_automation_probe(1500) == Some(true) {
            AUTOMATION_KNOWN_GRANTED.store(true, Ordering::Relaxed);
            write_automation_cache();
            log::debug!("automation: probe discovered grant (no prior cache)");
            return PermissionState::Granted;
        }
    }

    if AUTOMATION_KNOWN_GRANTED.load(Ordering::Relaxed) {
        PermissionState::Granted
    } else {
        PermissionState::Unknown
    }
}

/// Quick, non-interactive probe: spawns osascript with a configurable timeout.
/// Returns `Some(true)` if granted, `Some(false)` if denied, `None` if timed out.
fn quick_automation_probe(timeout_ms: u64) -> Option<bool> {
    let mut child = match Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to return name of first process"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Some(false),
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return Some(false),
        }
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
            Ok(Some(status)) => {
                let granted = status.success();
                if granted {
                    AUTOMATION_KNOWN_GRANTED.store(true, Ordering::Relaxed);
                    write_automation_cache();
                }
                return granted;
            }
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

/// Check if the app has App Management permission by spawning a subprocess that
/// writes inside a system-installed app bundle. A child process gets a fresh TCC
/// evaluation, bypassing any per-process access caching in the parent.
pub fn has_app_management() -> bool {
    let test_path = "/Applications/Safari.app/Contents/.macplus_probe";
    if !Path::new("/Applications/Safari.app/Contents").exists() {
        return false;
    }
    let status = Command::new("/usr/bin/touch")
        .arg(test_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => {
            let _ = std::fs::remove_file(test_path);
            true
        }
        _ => false,
    }
}
