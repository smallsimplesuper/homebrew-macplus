use std::io::{Read as _, Write as _};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::executor::sparkle_executor;
use crate::updaters::github_releases::check_github_release;
use crate::updaters::version_compare;
use crate::utils::brew::brew_path;
use crate::utils::AppError;

const SELF_REPO_OWNER: &str = "smallsimplesuper";
const SELF_REPO_NAME: &str = "macplus";
const SELF_BUNDLE_ID: &str = "com.macplus.app";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfUpdateInfo {
    pub available_version: String,
    pub current_version: String,
    pub release_notes_url: Option<String>,
    pub download_url: Option<String>,
    pub can_brew_upgrade: bool,
}

/// Standalone check that can be called from both the Tauri command and the scheduler.
pub async fn check_self_update_inner(client: &reqwest::Client) -> Option<SelfUpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION");

    let update = check_github_release(
        SELF_REPO_OWNER,
        SELF_REPO_NAME,
        SELF_BUNDLE_ID,
        Some(current_version),
        client,
    )
    .await
    .ok()
    .flatten()?;

    // Double-check: the version from GitHub must actually be newer
    if !version_compare::is_newer(current_version, &update.available_version) {
        return None;
    }

    // Check if macPlus is installed via Homebrew cask
    let can_brew_upgrade = check_brew_installed();

    Some(SelfUpdateInfo {
        available_version: update.available_version,
        current_version: current_version.to_string(),
        release_notes_url: update.release_notes_url,
        download_url: update.download_url,
        can_brew_upgrade,
    })
}

/// Check whether macPlus is installed as a Homebrew cask.
fn check_brew_installed() -> bool {
    let Some(brew) = brew_path() else {
        return false;
    };

    std::process::Command::new(brew.as_os_str())
        .current_dir("/tmp")
        .args(["list", "--cask", "macplus"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn check_self_update(
    http_client: State<'_, reqwest::Client>,
) -> Result<Option<SelfUpdateInfo>, AppError> {
    Ok(check_self_update_inner(http_client.inner()).await)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfUpdateProgress {
    pub phase: String,
    pub percent: u8,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

fn emit_progress(app: &AppHandle, phase: &str, percent: u8, dl: Option<u64>, total: Option<u64>) {
    let _ = app.emit(
        "self-update-progress",
        SelfUpdateProgress {
            phase: phase.to_string(),
            percent,
            downloaded_bytes: dl,
            total_bytes: total,
        },
    );
}

/// Check whether the app's parent directory is writable without elevation.
fn is_writable(path: &std::path::Path) -> bool {
    let parent = match path.parent() {
        Some(p) => p,
        None => return false,
    };
    let probe = parent.join(".macplus-write-test");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Shell script template for the detached updater.
///
/// Placeholders: {OLD_PID}, {OLD_APP}, {NEW_APP}, {NEEDS_SUDO}, {LOG}
const UPDATER_SCRIPT_TEMPLATE: &str = r#"#!/bin/bash
set -euo pipefail

LOG="{LOG}"
exec >> "$LOG" 2>&1

log() { echo "[$(date '+%H:%M:%S')] $*"; }

cleanup() {
    log "Cleanup: removing temp dir and self"
    rm -rf "{TMP_DIR}"
    rm -f "$0"
}
trap cleanup EXIT

OLD_PID={OLD_PID}
OLD_APP="{OLD_APP}"
NEW_APP="{NEW_APP}"
NEEDS_SUDO={NEEDS_SUDO}
BACKUP="${OLD_APP}.update-backup"

# 1. Wait for old process to exit
log "Waiting for PID $OLD_PID to exit..."
WAITED=0
while kill -0 "$OLD_PID" 2>/dev/null; do
    sleep 0.5
    WAITED=$((WAITED + 1))
    if [ "$WAITED" -ge 20 ]; then
        log "Old process still running after 10s, sending SIGKILL"
        kill -9 "$OLD_PID" 2>/dev/null || true
        sleep 1
        break
    fi
done
log "Old process exited"

# 2. Validate new app bundle
if [ ! -x "${NEW_APP}/Contents/MacOS/macPlus" ]; then
    log "ERROR: New app bundle is invalid (missing executable)"
    exit 1
fi

# 3. Replace app bundle
do_replace() {
    log "Backing up old app to $BACKUP"
    mv "$OLD_APP" "$BACKUP"

    log "Copying new app to $OLD_APP"
    cp -R "$NEW_APP" "$OLD_APP"

    log "Removing quarantine attribute"
    xattr -rd com.apple.quarantine "$OLD_APP" 2>/dev/null || true

    log "Removing backup"
    rm -rf "$BACKUP"
}

rollback() {
    log "ERROR: Replacement failed, rolling back from backup"
    if [ -d "$BACKUP" ]; then
        rm -rf "$OLD_APP" 2>/dev/null || true
        mv "$BACKUP" "$OLD_APP"
        log "Rollback complete"
    else
        log "No backup found, cannot rollback"
    fi
}

if [ "$NEEDS_SUDO" = "1" ]; then
    log "Elevated privileges required"
    # Try sudo -n first (reuses cached timestamp from pre-auth)
    if sudo -n sh -c "$(cat <<'INNER'
set -e
mv "$1" "$2"
cp -R "$3" "$1"
xattr -rd com.apple.quarantine "$1" 2>/dev/null || true
rm -rf "$2"
INNER
)" -- "$OLD_APP" "$BACKUP" "$NEW_APP" 2>/dev/null; then
        log "Replaced with sudo -n"
    else
        log "sudo -n failed, trying osascript"
        ESCAPED_OLD=$(echo "$OLD_APP" | sed "s/'/'\\\\''/g")
        ESCAPED_BACKUP=$(echo "$BACKUP" | sed "s/'/'\\\\''/g")
        ESCAPED_NEW=$(echo "$NEW_APP" | sed "s/'/'\\\\''/g")
        SCRIPT="mv '${ESCAPED_OLD}' '${ESCAPED_BACKUP}' && cp -R '${ESCAPED_NEW}' '${ESCAPED_OLD}' && xattr -rd com.apple.quarantine '${ESCAPED_OLD}' 2>/dev/null; rm -rf '${ESCAPED_BACKUP}'"
        if osascript -e "do shell script \"${SCRIPT}\" with administrator privileges" 2>/dev/null; then
            log "Replaced with osascript elevation"
        else
            log "osascript elevation failed"
            rollback
            exit 1
        fi
    fi
else
    if do_replace; then
        log "Replaced without elevation"
    else
        rollback
        exit 1
    fi
fi

# 4. Relaunch
log "Launching new app"
open "$OLD_APP"
log "Update complete"
"#;

#[tauri::command]
pub async fn execute_self_update(
    download_url: String,
    app_handle: AppHandle,
    http_client: State<'_, reqwest::Client>,
) -> Result<(), AppError> {
    // 1. Find current app path
    let exe = std::env::current_exe()
        .map_err(|e| AppError::CommandFailed(format!("Failed to find current executable: {}", e)))?;
    // exe is .app/Contents/MacOS/<binary> — go up 3 levels to get .app bundle
    let app_bundle = exe
        .parent() // MacOS/
        .and_then(|p| p.parent()) // Contents/
        .and_then(|p| p.parent()) // .app/
        .ok_or_else(|| AppError::CommandFailed("Failed to resolve .app bundle path".to_string()))?;
    let app_path_str = app_bundle.to_string_lossy().to_string();

    emit_progress(&app_handle, "Preparing update...", 2, None, None);

    // 2. Create stable temp dir (not RAII — the shell script handles cleanup)
    let pid = std::process::id();
    let tmp_dir = std::path::PathBuf::from(format!("/tmp/macplus-update-{}", pid));
    if tmp_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| AppError::CommandFailed(format!("Failed to create temp dir: {}", e)))?;

    // 3. Download DMG with streaming progress
    emit_progress(&app_handle, "Requesting download...", 5, None, None);

    let response = http_client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| AppError::CommandFailed(format!("Download failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::CommandFailed(format!(
            "Download returned HTTP {}",
            response.status()
        )));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let filename = response
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split("filename=").nth(1).map(|f| f.trim_matches('"').to_string()))
        .unwrap_or_else(|| {
            download_url
                .split('/')
                .last()
                .unwrap_or("update.dmg")
                .split('?')
                .next()
                .unwrap_or("update.dmg")
                .to_string()
        });

    let total_bytes = response.content_length();
    let download_path = tmp_dir.join(&filename);
    let mut file = std::fs::File::create(&download_path)
        .map_err(|e| AppError::CommandFailed(format!("Failed to create download file: {}", e)))?;
    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| AppError::CommandFailed(format!("Download stream error: {}", e)))?;
        file.write_all(&chunk)
            .map_err(|e| AppError::CommandFailed(format!("Failed to write chunk: {}", e)))?;
        downloaded += chunk.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(150) {
            last_emit = Instant::now();
            let pct = total_bytes
                .map(|t| ((downloaded as f64 / t as f64) * 100.0) as u8)
                .unwrap_or(0);
            // Map download progress to 5-50% range
            let mapped = 5 + (pct as u16 * 45 / 100) as u8;
            emit_progress(
                &app_handle,
                "Downloading update...",
                mapped,
                Some(downloaded),
                total_bytes,
            );
        }
    }
    drop(file);

    emit_progress(&app_handle, "Download complete, extracting...", 50, None, None);

    // 4. Detect file type
    let mut magic_buf = [0u8; 16];
    let magic_len = {
        let mut f = std::fs::File::open(&download_path)
            .map_err(|e| AppError::CommandFailed(format!("Failed to reopen download: {}", e)))?;
        f.read(&mut magic_buf)
            .map_err(|e| AppError::CommandFailed(format!("Failed to read magic bytes: {}", e)))?
    };
    let file_type = sparkle_executor::detect_file_type(&content_type, &filename, &magic_buf[..magic_len]);

    if file_type != sparkle_executor::FileType::Dmg {
        return Err(AppError::CommandFailed(format!(
            "Expected DMG file but detected {:?} for {}",
            file_type, filename
        )));
    }

    // 5. Extract from DMG
    let progress_cb = |pct: u8, phase: &str, _bytes: Option<(u64, Option<u64>)>| {
        emit_progress(&app_handle, phase, pct, None, None);
    };
    let new_app_path =
        sparkle_executor::extract_from_dmg(&download_path, &tmp_dir, &progress_cb, "macPlus")?;

    emit_progress(&app_handle, "Preparing to install...", 75, None, None);

    // 6. Check write access and pre-authenticate if needed
    let needs_sudo = !is_writable(app_bundle);
    if needs_sudo {
        emit_progress(&app_handle, "Requesting administrator privileges...", 80, None, None);
        if !crate::utils::sudo_session::pre_authenticate() {
            return Err(AppError::CommandFailed(
                "Update cancelled \u{2014} administrator approval is required".to_string(),
            ));
        }
    }

    emit_progress(&app_handle, "Installing update...", 85, None, None);

    // 7. Write updater shell script
    let script_path = format!("/tmp/macplus-self-update-{}.sh", pid);
    let log_path = "/tmp/macplus-self-update.log";
    let script_content = UPDATER_SCRIPT_TEMPLATE
        .replace("{OLD_PID}", &pid.to_string())
        .replace("{OLD_APP}", &app_path_str)
        .replace("{NEW_APP}", &new_app_path.to_string_lossy())
        .replace("{NEEDS_SUDO}", if needs_sudo { "1" } else { "0" })
        .replace("{TMP_DIR}", &tmp_dir.to_string_lossy())
        .replace("{LOG}", log_path);

    std::fs::write(&script_path, &script_content)
        .map_err(|e| AppError::CommandFailed(format!("Failed to write updater script: {}", e)))?;

    // Make executable
    Command::new("chmod")
        .current_dir("/tmp")
        .args(["+x", &script_path])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to chmod updater script: {}", e)))?;

    // 8. Spawn updater script fully detached (survives parent exit)
    emit_progress(&app_handle, "Relaunching macPlus...", 95, None, None);

    let mut cmd = Command::new("/bin/bash");
    cmd.current_dir("/tmp")
        .arg(&script_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // SAFETY: setsid() creates a new session so the script is not killed when our process exits.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    cmd.spawn()
        .map_err(|e| AppError::CommandFailed(format!("Failed to spawn updater script: {}", e)))?;

    // 9. Exit old process — the script takes it from here
    tokio::time::sleep(Duration::from_millis(200)).await;
    app_handle.exit(0);

    Ok(())
}
