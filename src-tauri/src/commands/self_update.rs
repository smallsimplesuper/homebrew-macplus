use std::io::{Read as _, Write as _};
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
const SELF_REPO_NAME: &str = "homebrew-macplus";
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

    // 2. Create temp dir
    let tmp_dir = tempfile::tempdir()
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
    let download_path = tmp_dir.path().join(&filename);
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
        sparkle_executor::extract_from_dmg(&download_path, tmp_dir.path(), &progress_cb, "macPlus")?;

    emit_progress(&app_handle, "Replacing application...", 75, None, None);

    // 6. Replace app bundle: rm -rf old + cp -R new
    let cp_output = Command::new("cp")
        .current_dir("/tmp")
        .args(["-R", &new_app_path.to_string_lossy(), &app_path_str])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to copy app: {}", e)))?;

    if !cp_output.status.success() {
        let stderr = String::from_utf8_lossy(&cp_output.stderr);
        let needs_elevation =
            stderr.contains("Permission denied") || stderr.contains("Operation not permitted");

        if needs_elevation {
            emit_progress(&app_handle, "Requesting administrator privileges...", 80, None, None);

            let elevated_cmd = format!(
                "rm -rf '{}' && cp -R '{}' '{}'",
                app_path_str.replace('\'', "'\\''"),
                new_app_path.to_string_lossy().replace('\'', "'\\''"),
                app_path_str.replace('\'', "'\\''"),
            );

            match crate::utils::sudo_session::run_elevated_shell(&elevated_cmd) {
                Ok(out) if out.status.success() => {}
                Err(crate::utils::sudo_session::ElevatedError::UserCancelled) => {
                    return Err(AppError::CommandFailed(
                        "Update cancelled \u{2014} administrator approval is required".to_string(),
                    ));
                }
                Ok(out) => {
                    let msg = String::from_utf8_lossy(&out.stderr).to_string();
                    return Err(AppError::CommandFailed(format!(
                        "Failed to replace app (elevated): {}",
                        msg
                    )));
                }
                Err(e) => {
                    return Err(AppError::CommandFailed(format!(
                        "Failed to request admin privileges: {}",
                        e
                    )));
                }
            }
        } else {
            return Err(AppError::CommandFailed(format!(
                "Failed to replace app: {}",
                stderr
            )));
        }
    }

    emit_progress(&app_handle, "Finishing up...", 90, None, None);

    // 7. Remove quarantine attribute (best-effort, try elevated if needed)
    let xattr_output = Command::new("xattr")
        .current_dir("/tmp")
        .args(["-rd", "com.apple.quarantine", &app_path_str])
        .output();
    if let Ok(ref out) = xattr_output {
        if !out.status.success() {
            let _ = crate::utils::sudo_session::run_elevated(
                "xattr",
                &["-rd", "com.apple.quarantine", &app_path_str],
            );
        }
    }

    emit_progress(&app_handle, "Relaunching macPlus...", 95, None, None);

    // 8. Launch new version
    let _ = Command::new("open")
        .current_dir("/tmp")
        .arg(&app_path_str)
        .spawn();

    // 9. Brief sleep then exit old process
    tokio::time::sleep(Duration::from_millis(500)).await;
    app_handle.exit(0);

    Ok(())
}
