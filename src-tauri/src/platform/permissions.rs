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
