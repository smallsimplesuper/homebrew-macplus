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

    // 2. Create stable temp dir
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

    // 7. Replace app bundle inline (no shell script)
    let old_app = app_bundle.to_path_buf();
    let backup = old_app.with_extension("update-backup");
    let new_app = new_app_path.clone();

    if needs_sudo {
        let old_app_s = old_app.to_string_lossy().to_string();
        let backup_s = backup.to_string_lossy().to_string();
        let new_app_s = new_app.to_string_lossy().to_string();

        let cmd = format!(
            "mv '{}' '{}' && cp -R '{}' '{}' && xattr -rd com.apple.quarantine '{}' 2>/dev/null; rm -rf '{}'",
            old_app_s, backup_s, new_app_s, old_app_s, old_app_s, backup_s
        );
        let cmd_clone = cmd.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::utils::sudo_session::run_elevated_shell(&cmd_clone)
        })
        .await
        .map_err(|e| AppError::CommandFailed(format!("spawn_blocking failed: {}", e)))?;

        match result {
            Ok(output) if output.status.success() => {
                log::info!("Self-update: replaced app bundle with sudo");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Rollback
                if backup.exists() {
                    let _ = std::fs::remove_dir_all(&old_app);
                    let _ = std::fs::rename(&backup, &old_app);
                }
                return Err(AppError::CommandFailed(format!(
                    "Elevated replacement failed: {}",
                    stderr
                )));
            }
            Err(e) => {
                // Rollback
                if backup.exists() {
                    let _ = std::fs::remove_dir_all(&old_app);
                    let _ = std::fs::rename(&backup, &old_app);
                }
                return Err(AppError::CommandFailed(format!(
                    "Elevated replacement failed: {}",
                    e
                )));
            }
        }
    } else {
        // Non-sudo path
        std::fs::rename(&old_app, &backup)
            .map_err(|e| AppError::CommandFailed(format!("Failed to backup old app: {}", e)))?;

        let new_app_str = new_app.to_string_lossy().to_string();
        let old_app_str = old_app.to_string_lossy().to_string();

        let cp_result = Command::new("cp")
            .args(["-R", &new_app_str, &old_app_str])
            .output();

        match cp_result {
            Ok(output) if output.status.success() => {
                // Remove quarantine
                Command::new("xattr")
                    .args(["-rd", "com.apple.quarantine", &old_app_str])
                    .output()
                    .ok();
                // Remove backup
                std::fs::remove_dir_all(&backup).ok();
                log::info!("Self-update: replaced app bundle without elevation");
            }
            _ => {
                // Rollback
                let _ = std::fs::remove_dir_all(&old_app);
                let _ = std::fs::rename(&backup, &old_app);
                return Err(AppError::CommandFailed(
                    "Failed to copy new app bundle".to_string(),
                ));
            }
        }
    }

    // Verify the new binary exists
    let new_binary = old_app.join("Contents/MacOS/macPlus");
    if !new_binary.exists() {
        log::error!("Self-update: new binary not found at {:?}", new_binary);
        return Err(AppError::CommandFailed(
            "Update failed: new binary not found after replacement".to_string(),
        ));
    }

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&tmp_dir);

    emit_progress(&app_handle, "Relaunching macPlus...", 95, None, None);

    // 8. Write tiny relaunch script and spawn detached
    let script_path = format!("/tmp/macplus-relaunch-{}.sh", pid);
    let script_content = format!(
        "#!/bin/bash\nsleep 1\nopen '{}'\nrm -f \"$0\"\n",
        app_path_str
    );
    std::fs::write(&script_path, &script_content)
        .map_err(|e| AppError::CommandFailed(format!("Failed to write relaunch script: {}", e)))?;
    Command::new("chmod")
        .current_dir("/tmp")
        .args(["+x", &script_path])
        .output()
        .ok();

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
        .map_err(|e| AppError::CommandFailed(format!("Failed to spawn relaunch script: {}", e)))?;

    // 9. Exit old process — the script relaunches
    tokio::time::sleep(Duration::from_millis(200)).await;
    app_handle.exit(0);

    Ok(())
}
