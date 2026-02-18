use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::{AssociatedFile, AssociatedFiles, UninstallProgress, UninstallResult};
use crate::utils::brew::{brew_command, brew_path};
use crate::utils::sudo_session::run_elevated_shell;
use crate::utils::AppError;

fn emit_uninstall_progress(app: &AppHandle, phase: &str, percent: u8) {
    let _ = app.emit(
        "uninstall-progress",
        UninstallProgress {
            phase: phase.to_string(),
            percent,
        },
    );
}

/// Move a path to Trash via Finder AppleScript (reversible).
fn move_to_trash(path: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .current_dir("/tmp")
        .args([
            "-e",
            &format!(
                "tell application \"Finder\" to move POSIX file \"{}\" to trash",
                path.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Finder trash failed: {}", stderr.trim()))
    }
}

/// Move a path to Trash with elevated privileges.
fn move_to_trash_elevated(path: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"Finder\" to move POSIX file \"{}\" to trash",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let shell_cmd = format!("osascript -e '{}'", script.replace('\'', "'\\''"));
    run_elevated_shell(&shell_cmd)
        .map_err(|e| format!("Elevated trash failed: {}", e))?;
    Ok(())
}

/// Check if an app is currently running by its app path.
fn is_app_running(app_path: &str) -> bool {
    let output = Command::new("pgrep")
        .current_dir("/tmp")
        .args(["-f", app_path])
        .output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Compute the total size of a directory or file in bytes.
fn path_size(path: &Path) -> u64 {
    if path.is_file() {
        path.metadata().map(|m| m.len()).unwrap_or(0)
    } else if path.is_dir() {
        walkdir_size(path)
    } else {
        0
    }
}

fn walkdir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let ft = entry.file_type();
            if let Ok(ft) = ft {
                if ft.is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                } else if ft.is_dir() {
                    total += walkdir_size(&entry.path());
                }
            }
        }
    }
    total
}

/// Scan ~/Library subdirectories for files associated with a bundle_id/display_name.
fn find_associated_files(bundle_id: &str, display_name: &str) -> Vec<AssociatedFile> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let library = home.join("Library");
    let mut files = Vec::new();

    let search_patterns: Vec<(&str, Vec<String>)> = vec![
        (
            "application_support",
            vec![
                library.join("Application Support").join(display_name).to_string_lossy().to_string(),
                library.join("Application Support").join(bundle_id).to_string_lossy().to_string(),
            ],
        ),
        (
            "preferences",
            vec![], // handled via glob below
        ),
        (
            "caches",
            vec![library.join("Caches").join(bundle_id).to_string_lossy().to_string()],
        ),
        (
            "http_storages",
            vec![library.join("HTTPStorages").join(bundle_id).to_string_lossy().to_string()],
        ),
        (
            "saved_state",
            vec![
                library
                    .join("Saved Application State")
                    .join(format!("{}.savedState", bundle_id))
                    .to_string_lossy()
                    .to_string(),
            ],
        ),
        (
            "containers",
            vec![library.join("Containers").join(bundle_id).to_string_lossy().to_string()],
        ),
        (
            "logs",
            vec![library.join("Logs").join(display_name).to_string_lossy().to_string()],
        ),
        (
            "webkit",
            vec![library.join("WebKit").join(bundle_id).to_string_lossy().to_string()],
        ),
    ];

    for (kind, paths) in &search_patterns {
        for path_str in paths {
            let path = Path::new(path_str);
            if path.exists() {
                let size = path_size(path);
                files.push(AssociatedFile {
                    path: path_str.clone(),
                    size_bytes: size,
                    kind: kind.to_string(),
                });
            }
        }
    }

    // Preferences: glob for bundle_id plist files
    let prefs_dir = library.join("Preferences");
    if prefs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&prefs_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(bundle_id) && name.ends_with(".plist") {
                    let full_path = entry.path();
                    let size = full_path.metadata().map(|m| m.len()).unwrap_or(0);
                    files.push(AssociatedFile {
                        path: full_path.to_string_lossy().to_string(),
                        size_bytes: size,
                        kind: "preferences".to_string(),
                    });
                }
            }
        }
    }

    // Group Containers: match *.<bundleId>
    let group_containers = library.join("Group Containers");
    if group_containers.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&group_containers) {
            let suffix = format!(".{}", bundle_id);
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(&suffix) {
                    let full_path = entry.path();
                    let size = path_size(&full_path);
                    files.push(AssociatedFile {
                        path: full_path.to_string_lossy().to_string(),
                        size_bytes: size,
                        kind: "group_containers".to_string(),
                    });
                }
            }
        }
    }

    files
}

/// Uninstall an app by bundle_id.
fn uninstall_homebrew_cask(token: &str) -> Result<String, String> {
    let brew = brew_path().ok_or("Homebrew not found")?;

    // Standard uninstall
    let output = brew_command(brew)
        .args(["uninstall", "--cask", token])
        .output()
        .map_err(|e| format!("Failed to run brew: {}", e))?;

    if output.status.success() {
        // Cleanup
        let _ = brew_command(brew).arg("cleanup").output();
        return Ok(format!("Successfully uninstalled cask {}", token));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Permission error — retry with elevation
    if stderr.contains("Permission denied") || stderr.contains("EPERM") {
        let cmd = format!("{} uninstall --cask {}", brew.display(), token);
        match run_elevated_shell(&cmd) {
            Ok(elevated_output) => {
                if elevated_output.status.success() {
                    let _ = brew_command(brew).arg("cleanup").output();
                    return Ok(format!("Successfully uninstalled cask {} (elevated)", token));
                }
            }
            Err(e) => return Err(format!("Elevated uninstall failed: {}", e)),
        }
    }

    // Retry with --force
    let force_output = brew_command(brew)
        .args(["uninstall", "--cask", "--force", token])
        .output()
        .map_err(|e| format!("Failed to run brew --force: {}", e))?;

    if force_output.status.success() {
        let _ = brew_command(brew).arg("cleanup").output();
        return Ok(format!("Successfully force-uninstalled cask {}", token));
    }

    let force_stderr = String::from_utf8_lossy(&force_output.stderr);
    Err(format!("brew uninstall --cask failed: {}", force_stderr.trim()))
}

fn uninstall_homebrew_formula(name: &str) -> Result<String, String> {
    let brew = brew_path().ok_or("Homebrew not found")?;

    let output = brew_command(brew)
        .args(["uninstall", name])
        .output()
        .map_err(|e| format!("Failed to run brew: {}", e))?;

    if output.status.success() {
        let _ = brew_command(brew).arg("cleanup").output();
        return Ok(format!("Successfully uninstalled formula {}", name));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Permission error — retry with elevation
    if stderr.contains("Permission denied") || stderr.contains("EPERM") {
        let cmd = format!("{} uninstall {}", brew.display(), name);
        match run_elevated_shell(&cmd) {
            Ok(elevated_output) => {
                if elevated_output.status.success() {
                    let _ = brew_command(brew).arg("cleanup").output();
                    return Ok(format!("Successfully uninstalled formula {} (elevated)", name));
                }
            }
            Err(e) => return Err(format!("Elevated uninstall failed: {}", e)),
        }
    }

    // Retry with --force
    let force_output = brew_command(brew)
        .args(["uninstall", "--force", name])
        .output()
        .map_err(|e| format!("Failed to run brew --force: {}", e))?;

    if force_output.status.success() {
        let _ = brew_command(brew).arg("cleanup").output();
        return Ok(format!("Successfully force-uninstalled formula {}", name));
    }

    let force_stderr = String::from_utf8_lossy(&force_output.stderr);
    Err(format!("brew uninstall failed: {}", force_stderr.trim()))
}

#[tauri::command]
pub async fn scan_associated_files(bundle_id: String) -> Result<AssociatedFiles, AppError> {
    // Use bundle_id's last component as fallback display_name
    let display_name = bundle_id
        .rsplit('.')
        .next()
        .unwrap_or(&bundle_id)
        .to_string();

    let files =
        tokio::task::spawn_blocking(move || find_associated_files(&bundle_id, &display_name))
            .await
            .map_err(|e| AppError::Custom(format!("Scan task failed: {}", e)))?;

    let total_size_bytes = files.iter().map(|f| f.size_bytes).sum();

    Ok(AssociatedFiles {
        paths: files,
        total_size_bytes,
    })
}

#[tauri::command]
pub async fn uninstall_app(
    app_handle: AppHandle,
    bundle_id: String,
    cleanup_associated: bool,
) -> Result<UninstallResult, AppError> {
    // Safety check: block system apps
    let db = app_handle.state::<Arc<Mutex<Database>>>();
    let (display_name, app_path, homebrew_cask_token, homebrew_formula_name, icon_cache_path) = {
        let db_guard = db.lock().await;
        let detail = db_guard.get_app_detail(&bundle_id)?;
        (
            detail.display_name,
            detail.app_path,
            detail.homebrew_cask_token,
            detail.homebrew_formula_name,
            detail.icon_cache_path,
        )
    };

    // Block system apps
    if app_path.starts_with("/System/Applications/") || app_path.starts_with("/System/Library/") {
        return Ok(UninstallResult {
            bundle_id,
            success: false,
            message: Some("System apps cannot be uninstalled.".to_string()),
            running: false,
            cleaned_paths: Vec::new(),
            protected: true,
        });
    }

    // Block self-uninstall
    if bundle_id == "com.macplus.app" {
        return Ok(UninstallResult {
            bundle_id,
            success: false,
            message: Some("macPlus cannot uninstall itself.".to_string()),
            running: false,
            cleaned_paths: Vec::new(),
            protected: true,
        });
    }

    // Check if running
    let app_path_clone = app_path.clone();
    let running =
        tokio::task::spawn_blocking(move || is_app_running(&app_path_clone))
            .await
            .unwrap_or(false);

    if running {
        return Ok(UninstallResult {
            bundle_id,
            success: false,
            message: Some(format!("{} is currently running. Quit it first, then try again.", display_name)),
            running: true,
            cleaned_paths: Vec::new(),
            protected: false,
        });
    }

    // Route to uninstall method
    emit_uninstall_progress(&app_handle, "Preparing...", 0);
    emit_uninstall_progress(&app_handle, &format!("Uninstalling {}...", display_name), 20);

    let uninstall_result = if let Some(ref token) = homebrew_cask_token {
        let token = token.clone();
        tokio::task::spawn_blocking(move || uninstall_homebrew_cask(&token)).await
    } else if let Some(ref name) = homebrew_formula_name {
        let name = name.clone();
        tokio::task::spawn_blocking(move || uninstall_homebrew_formula(&name)).await
    } else {
        // Direct / MAS / unknown — move .app to Trash via Finder
        let path = app_path.clone();
        tokio::task::spawn_blocking(move || {
            match move_to_trash(&path) {
                Ok(()) => Ok(format!("Moved {} to Trash", path)),
                Err(_) => {
                    // Retry with elevation
                    move_to_trash_elevated(&path)
                        .map(|()| format!("Moved {} to Trash (elevated)", path))
                }
            }
        })
        .await
    };

    let (success, message) = match uninstall_result {
        Ok(Ok(msg)) => (true, Some(msg)),
        Ok(Err(err)) => (false, Some(err)),
        Err(e) => (false, Some(format!("Task failed: {}", e))),
    };

    let phase_msg = if success {
        format!("Uninstalled {}", display_name)
    } else {
        "Uninstall failed".to_string()
    };
    emit_uninstall_progress(&app_handle, &phase_msg, 50);

    // Associated file cleanup
    let mut cleaned_paths = Vec::new();
    if success && cleanup_associated {
        emit_uninstall_progress(&app_handle, "Scanning associated files...", 55);
        let bid = bundle_id.clone();
        let dname = display_name.clone();
        let associated =
            tokio::task::spawn_blocking(move || find_associated_files(&bid, &dname))
                .await
                .unwrap_or_default();

        let file_count = associated.len();
        for (i, file) in associated.iter().enumerate() {
            let pct = 60 + ((i as u8) * 25 / (file_count.max(1) as u8)).min(25);
            let short_path = file.path.rsplit('/').next().unwrap_or(&file.path);
            emit_uninstall_progress(&app_handle, &format!("Cleaning up {}...", short_path), pct);
            let path = file.path.clone();
            let result = tokio::task::spawn_blocking(move || move_to_trash(&path)).await;
            if let Ok(Ok(())) = result {
                cleaned_paths.push(file.path.clone());
            }
        }
    }

    // Database cleanup
    emit_uninstall_progress(&app_handle, "Cleaning database...", 90);
    if success {
        let db_guard = db.lock().await;
        let _ = db_guard.delete_app(&bundle_id);

        // Clean up icon cache file
        if let Some(icon_path) = &icon_cache_path {
            let _ = std::fs::remove_file(icon_path);
        }
    }

    emit_uninstall_progress(&app_handle, "Complete", 100);

    // Native notification
    if success {
        use tauri_plugin_notification::NotificationExt;
        let db_guard = db.lock().await;
        let settings = crate::scheduler::load_settings_from_db(&db_guard);
        drop(db_guard);

        if settings.notification_on_updates {
            let mut builder = app_handle
                .notification()
                .builder()
                .title("macPlus")
                .body(&format!("{} has been uninstalled", display_name));
            if settings.notification_sound {
                builder = builder.sound("Glass");
            }
            let _ = builder.show();
        }
    }

    // Emit event
    let _ = app_handle.emit(
        "app-uninstalled",
        serde_json::json!({
            "bundleId": bundle_id,
            "displayName": display_name,
            "success": success,
            "cleanedPaths": cleaned_paths,
        }),
    );

    Ok(UninstallResult {
        bundle_id,
        success,
        message,
        running: false,
        cleaned_paths,
        protected: false,
    })
}
