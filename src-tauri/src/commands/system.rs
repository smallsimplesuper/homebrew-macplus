use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State};
use std::path::Path;
use std::process::Command;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::platform::{icon_extractor, permissions};
use crate::utils::askpass;
use crate::utils::brew;
use crate::utils::{self, AppError};

// ---------------------------------------------------------------------------
// Connectivity health check
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectivityStatus {
    pub github: bool,
    pub homebrew: bool,
    pub itunes: bool,
    pub overall: String,
}

#[tauri::command]
pub async fn check_connectivity(
    http_client: State<'_, reqwest::Client>,
) -> Result<ConnectivityStatus, AppError> {
    Ok(check_connectivity_inner(http_client.inner()).await)
}

async fn ping_url(client: &reqwest::Client, url: &str, timeout: std::time::Duration) -> bool {
    let result = client
        .head(url)
        .timeout(timeout)
        .send()
        .await;
    result.is_ok()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsStatus {
    pub automation: bool,
    pub automation_state: String,
    pub full_disk_access: bool,
    pub app_management: bool,
    pub notifications: bool,
}

#[tauri::command]
pub async fn get_permissions_status() -> Result<PermissionsStatus, AppError> {
    let (automation_state, full_disk_access, app_management, notifications) = tokio::join!(
        tokio::task::spawn_blocking(permissions::check_automation_passive),
        tokio::task::spawn_blocking(permissions::has_full_disk_access),
        tokio::task::spawn_blocking(permissions::has_app_management),
        tokio::task::spawn_blocking(|| permissions::has_notification_permission("com.macplus.app")),
    );
    let automation_state = automation_state.unwrap_or(permissions::PermissionState::Unknown);
    let full_disk_access = full_disk_access.unwrap_or(false);
    let app_management = app_management.unwrap_or(false);
    let notifications = notifications.unwrap_or(false);

    Ok(PermissionsStatus {
        automation: automation_state.is_granted(),
        automation_state: automation_state.as_str().to_string(),
        full_disk_access,
        app_management,
        notifications,
    })
}

/// Lightweight passive-only permission check. No dialogs, no connectivity checks,
/// no Homebrew detection. Used by the PermissionBanner on mount and visibility changes.
#[tauri::command]
pub async fn get_permissions_passive() -> Result<PermissionsStatus, AppError> {
    let (automation_state, full_disk_access, app_management, notifications) = tokio::join!(
        tokio::task::spawn_blocking(permissions::check_automation_passive),
        tokio::task::spawn_blocking(permissions::has_full_disk_access),
        tokio::task::spawn_blocking(permissions::has_app_management),
        tokio::task::spawn_blocking(|| permissions::has_notification_permission("com.macplus.app")),
    );
    let automation_state = automation_state.unwrap_or(permissions::PermissionState::Unknown);
    let full_disk_access = full_disk_access.unwrap_or(false);
    let app_management = app_management.unwrap_or(false);
    let notifications = notifications.unwrap_or(false);

    Ok(PermissionsStatus {
        automation: automation_state.is_granted(),
        automation_state: automation_state.as_str().to_string(),
        full_disk_access,
        app_management,
        notifications,
    })
}

/// Intentionally trigger the macOS Automation permission dialog.
/// Called ONLY when the user clicks "Enable" for Automation in the PermissionBanner.
#[tauri::command]
pub async fn trigger_automation_permission() -> Result<bool, AppError> {
    let granted = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(permissions::trigger_automation_permission),
    )
    .await
    .unwrap_or(Ok(false))
    .unwrap_or(false);
    Ok(granted)
}

#[tauri::command]
pub async fn open_system_preferences(pane: String) -> Result<(), AppError> {
    let url = match pane.as_str() {
        "automation" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation",
        "full_disk_access" => "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
        "app_management" => "x-apple.systempreferences:com.apple.preference.security?Privacy_AppManagement",
        "notifications" => "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
        _ => return Err(AppError::CommandFailed(format!("Unknown pane: {}", pane))),
    };

    Command::new("open")
        .arg(url)
        .output()
        .map_err(|e| AppError::CommandFailed(format!("open preferences: {}", e)))?;
    Ok(())
}

#[tauri::command]
pub async fn open_app(path: String) -> Result<(), AppError> {
    Command::new("open")
        .arg(&path)
        .output()
        .map_err(|e| AppError::CommandFailed(format!("open: {}", e)))?;
    Ok(())
}

#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), AppError> {
    Command::new("open")
        .args(["-R", &path])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("open -R: {}", e)))?;
    Ok(())
}

#[tauri::command]
pub async fn get_app_icon(
    app_path: String,
    bundle_id: String,
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Option<String>, AppError> {
    let cache_dir = app_handle
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    let icons_dir = cache_dir.join("icons");
    std::fs::create_dir_all(&icons_dir)?;

    let result = icon_extractor::extract_icon_png(Path::new(&app_path), &icons_dir)?;

    if let Some(ref icon_path) = result {
        let db_guard = db.lock().await;
        let _ = db_guard.update_icon_cache_path(&bundle_id, icon_path);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Setup status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub homebrew_installed: bool,
    pub homebrew_version: Option<String>,
    pub homebrew_path: Option<String>,
    pub askpass_installed: bool,
    pub askpass_path: Option<String>,
    pub xcode_clt_installed: bool,
    pub permissions: PermissionsStatus,
    pub connectivity: ConnectivityStatus,
}

/// Run a command with a timeout (seconds). Returns first line of stdout on success.
fn run_with_timeout(program: &Path, args: &[&str], timeout_secs: u64) -> Option<String> {
    let mut child = Command::new(program)
        .current_dir("/tmp")
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let output = child.stdout.take()?;
                    let reader = std::io::BufReader::new(output);
                    use std::io::BufRead;
                    return reader.lines().next()?.ok().map(|l| l.trim().to_string());
                }
                return None;
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
}

#[tauri::command]
pub async fn check_setup_status(
    http_client: State<'_, reqwest::Client>,
) -> Result<SetupStatus, AppError> {
    let client = http_client.inner().clone();
    let timeout_dur = std::time::Duration::from_secs(15);

    let result = tokio::time::timeout(timeout_dur, async {
        // Run independent checks in parallel
        let (brew_result, automation_state, xcode, fda, app_mgmt, notif, connectivity) = tokio::join!(
            // Homebrew: version + path (blocking shell call)
            tokio::task::spawn_blocking(|| {
                let brew_installed = brew::brew_path().is_some();
                let brew_version = if brew_installed {
                    brew::brew_path().and_then(|p| run_with_timeout(p, &["--version"], 3))
                } else {
                    None
                };
                let brew_path_str = brew::brew_path().map(|p| p.display().to_string());
                (brew_installed, brew_version, brew_path_str)
            }),
            // Automation permission (passive TCC.db read — no dialog)
            tokio::task::spawn_blocking(permissions::check_automation_passive),
            // Xcode CLT (blocking shell call)
            tokio::task::spawn_blocking(utils::is_xcode_clt_installed),
            // Full Disk Access (subprocess check)
            tokio::task::spawn_blocking(permissions::has_full_disk_access),
            // App Management (subprocess check)
            tokio::task::spawn_blocking(permissions::has_app_management),
            // Notification permission (blocking plist check)
            tokio::task::spawn_blocking(|| {
                permissions::has_notification_permission("com.macplus.app")
            }),
            // Connectivity (async HTTP pings)
            check_connectivity_inner(&client),
        );

        let (brew_installed, brew_version, brew_path_str) = brew_result.unwrap_or((false, None, None));
        let automation_state = automation_state.unwrap_or(permissions::PermissionState::Unknown);
        let xcode_clt = xcode.unwrap_or(false);
        let fda = fda.unwrap_or(false);
        let app_mgmt = app_mgmt.unwrap_or(false);
        let notifications = notif.unwrap_or(false);

        let ap_installed = askpass::is_askpass_installed();
        let ap_path = askpass::askpass_path().map(|p| p.display().to_string());

        SetupStatus {
            homebrew_installed: brew_installed,
            homebrew_version: brew_version,
            homebrew_path: brew_path_str,
            askpass_installed: ap_installed,
            askpass_path: ap_path,
            xcode_clt_installed: xcode_clt,
            permissions: PermissionsStatus {
                automation: automation_state.is_granted(),
                automation_state: automation_state.as_str().to_string(),
                full_disk_access: fda,
                app_management: app_mgmt,
                notifications,
            },
            connectivity,
        }
    })
    .await;

    match result {
        Ok(status) => Ok(status),
        Err(_) => Err(AppError::Custom("Setup check timed out".to_string())),
    }
}

/// Internal connectivity check reusable by both `check_connectivity` and `check_setup_status`.
async fn check_connectivity_inner(client: &reqwest::Client) -> ConnectivityStatus {
    let timeout = std::time::Duration::from_secs(3);
    let (github, homebrew, itunes) = tokio::join!(
        ping_url(client, "https://api.github.com", timeout),
        ping_url(client, "https://formulae.brew.sh/api/cask.json", timeout),
        ping_url(client, "https://itunes.apple.com/lookup?bundleId=com.apple.Safari", timeout),
    );
    let reachable = [github, homebrew, itunes].iter().filter(|&&v| v).count();
    let overall = match reachable {
        3 => "connected",
        0 => "disconnected",
        _ => "partial",
    }
    .to_string();
    ConnectivityStatus { github, homebrew, itunes, overall }
}

#[tauri::command]
pub async fn ensure_askpass_helper(
    app_handle: tauri::AppHandle,
) -> Result<Option<String>, AppError> {
    // If already initialized and available, just return the path.
    if let Some(p) = askpass::askpass_path() {
        return Ok(Some(p.display().to_string()));
    }

    // Try to (re-)initialize from resource dir.
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| AppError::CommandFailed(format!("resource_dir: {}", e)))?;

    askpass::init_askpass_path(resource_dir);

    Ok(askpass::askpass_path().map(|p| p.display().to_string()))
}

#[tauri::command]
pub async fn open_terminal_with_command(command: String) -> Result<(), AppError> {
    let script = format!(
        r#"tell application "Terminal"
    activate
    do script "{}"
end tell"#,
        command.replace('"', "\\\"")
    );

    Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("osascript: {}", e)))?;
    Ok(())
}
