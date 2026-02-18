use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::executor::{
    delegated_executor::DelegatedExecutor, homebrew_executor::HomebrewExecutor,
    homebrew_formula_executor::HomebrewFormulaExecutor,
    mas_executor::MasExecutor, microsoft_autoupdate_executor::MicrosoftAutoUpdateExecutor,
    sparkle_executor::SparkleExecutor, UpdateExecutor,
};
use crate::models::{AppDetail, AppSource, UpdateExecuteComplete, UpdateExecuteProgress, UpdateResult};
use crate::utils::{app_lifecycle, sudo_session, AppError};

/// Truncate long hex-only version strings (e.g. commit hashes) for display.
fn truncate_version(version: &str) -> &str {
    if version.len() > 20 && version.chars().all(|c| c.is_ascii_hexdigit()) {
        version.get(..12).unwrap_or(version)
    } else {
        version
    }
}

/// Record the outcome of an update in the history table.
fn record_update_result(db: &Database, history_id: i64, result: &UpdateResult) {
    if result.delegated {
        let _ = db.record_update_delegated(history_id);
    } else if result.success {
        let _ = db.record_update_complete(history_id);
    } else {
        let _ = db.record_update_failed(history_id, result.message.as_deref().unwrap_or("Unknown error"));
    }
}

/// Check whether a URL points to a directly downloadable installer file.
fn is_downloadable_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.ends_with(".dmg") || lower.ends_with(".zip") || lower.ends_with(".pkg")
        || lower.contains(".dmg?") || lower.contains(".zip?") || lower.contains(".pkg?")
}

/// Route to the correct executor based on the available update's source_type,
/// falling back to install_source-based routing when no update info is present.
async fn route_and_execute(
    detail: &AppDetail,
    bundle_id: &str,
    on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
) -> Result<UpdateResult, AppError> {
    // Primary routing: by available_update.source_type
    if let Some(ref update) = detail.available_update {
        match update.source_type.as_str() {
            "homebrew_cask" => {
                // Try direct download first (no brew CLI needed)
                if let Some(ref url) = update.download_url {
                    if is_downloadable_url(url) {
                        return SparkleExecutor::new(url.clone(), detail.display_name.clone())
                            .with_source_type("homebrew_cask")
                            .execute(bundle_id, &detail.app_path, on_progress)
                            .await;
                    }
                }
                // Fallback: use Homebrew CLI
                if let Some(ref token) = detail.homebrew_cask_token {
                    return HomebrewExecutor::new(token.clone())
                        .with_pre_version(detail.installed_version.clone())
                        .execute(bundle_id, &detail.app_path, on_progress)
                        .await;
                }
            }
            "adobe_cc" => {
                // Open Adobe Creative Cloud for the user to apply updates
                let _ = std::process::Command::new("open")
                    .arg("-b")
                    .arg("com.adobe.acc.AdobeCreativeCloud")
                    .output();
                return Ok(UpdateResult {
                    bundle_id: bundle_id.to_string(),
                    success: true,
                    message: Some("Opened Adobe Creative Cloud to apply updates".to_string()),
                    source_type: "adobe_cc".to_string(),
                    from_version: detail.installed_version.clone(),
                    to_version: detail.available_update.as_ref().map(|u| u.available_version.clone()),
                    handled_relaunch: false,
                    delegated: true,
                });
            }
            "mas" => {
                return MasExecutor::new(detail.mas_app_id.clone())
                    .with_pre_version(detail.installed_version.clone())
                    .execute(bundle_id, &detail.app_path, on_progress)
                    .await;
            }
            "sparkle" => {
                if let Some(ref url) = update.download_url {
                    if is_downloadable_url(url) {
                        return SparkleExecutor::new(url.clone(), detail.display_name.clone())
                            .execute(bundle_id, &detail.app_path, on_progress)
                            .await;
                    }
                    // URL doesn't look like a downloadable file — fall through
                }
                // No download URL or non-downloadable URL — fall through to delegated/homebrew
            }
            "github" | "homebrew_api" => {
                // If the app has a direct download URL, use SparkleExecutor
                if let Some(ref url) = update.download_url {
                    if is_downloadable_url(url) {
                        let source = if update.source_type.as_str() == "homebrew_api" { "homebrew_api" } else { "github" };
                        return SparkleExecutor::new(url.clone(), detail.display_name.clone())
                            .with_source_type(source)
                            .execute(bundle_id, &detail.app_path, on_progress)
                            .await;
                    }
                }
                // If the app has a homebrew cask token, use HomebrewExecutor.
                if let Some(ref token) = detail.homebrew_cask_token {
                    return HomebrewExecutor::new(token.clone())
                        .with_pre_version(detail.installed_version.clone())
                        .execute(bundle_id, &detail.app_path, on_progress)
                        .await;
                }
                // Fallback to delegated (opens release page)
            }
            "microsoft_autoupdate" => {
                return MicrosoftAutoUpdateExecutor::new(detail.display_name.clone())
                    .with_cask_token(detail.homebrew_cask_token.clone())
                    .with_pre_version(detail.installed_version.clone())
                    .execute(bundle_id, &detail.app_path, on_progress)
                    .await;
            }
            _ => {}
        }
    }

    // Fallback routing: by install_source
    let source = AppSource::from_str(&detail.install_source);
    match source {
        AppSource::HomebrewFormula => {
            if let Some(ref name) = detail.homebrew_formula_name {
                HomebrewFormulaExecutor::new(name.clone())
                    .with_pre_version(detail.installed_version.clone())
                    .execute(bundle_id, &detail.app_path, on_progress)
                    .await
            } else {
                DelegatedExecutor::new()
                    .execute(bundle_id, &detail.app_path, on_progress)
                    .await
            }
        }
        AppSource::Homebrew => {
            if let Some(ref token) = detail.homebrew_cask_token {
                HomebrewExecutor::new(token.clone())
                    .with_pre_version(detail.installed_version.clone())
                    .execute(bundle_id, &detail.app_path, on_progress)
                    .await
            } else {
                DelegatedExecutor::new()
                    .execute(bundle_id, &detail.app_path, on_progress)
                    .await
            }
        }
        AppSource::MacAppStore => {
            MasExecutor::new(detail.mas_app_id.clone())
                .with_pre_version(detail.installed_version.clone())
                .execute(bundle_id, &detail.app_path, on_progress)
                .await
        }
        _ => {
            DelegatedExecutor::new()
                .execute(bundle_id, &detail.app_path, on_progress)
                .await
        }
    }
}

#[tauri::command]
pub async fn execute_update(
    bundle_id: String,
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<UpdateResult, AppError> {
    let db_guard = db.lock().await;
    let detail = db_guard.get_app_detail(&bundle_id)?;

    // Record history start
    let to_version_raw = detail.available_update.as_ref()
        .map(|u| u.available_version.as_str())
        .unwrap_or("unknown");
    let to_version = truncate_version(to_version_raw);
    let history_id = db_guard.record_update_start(
        detail.id,
        detail.installed_version.as_deref().unwrap_or("unknown"),
        to_version,
        &detail.install_source,
    ).ok();
    drop(db_guard);

    let handle = app_handle.clone();
    let bid = bundle_id.clone();

    let on_progress = move |percent: u8, phase: &str, bytes: Option<(u64, Option<u64>)>| {
        let _ = handle.emit(
            "update-execute-progress",
            UpdateExecuteProgress {
                bundle_id: bid.clone(),
                phase: phase.to_string(),
                percent,
                downloaded_bytes: bytes.map(|(d, _)| d),
                total_bytes: bytes.and_then(|(_, t)| t),
            },
        );
    };

    let result = route_and_execute(&detail, &bundle_id, &on_progress).await?;

    // Record history result
    if let Some(hid) = history_id {
        let db_guard = db.lock().await;
        record_update_result(&db_guard, hid, &result);
    }

    // Check if app needs relaunch (skip if the executor already handled it)
    let needs_relaunch = result.success
        && !result.handled_relaunch
        && (result.source_type == "homebrew_cask" || result.source_type == "homebrew_formula")
        && app_lifecycle::is_app_running(&bundle_id);

    let _ = app_handle.emit(
        "update-execute-complete",
        UpdateExecuteComplete {
            bundle_id: bundle_id.clone(),
            display_name: detail.display_name.clone(),
            success: result.success,
            message: result.message.clone(),
            needs_relaunch,
            app_path: if needs_relaunch { Some(detail.app_path.clone()) } else { None },
            delegated: result.delegated,
        },
    );

    // Send native notification for completed updates
    {
        let db_guard = db.lock().await;
        let settings = crate::scheduler::load_settings_from_db(&db_guard);
        drop(db_guard);

        if settings.notification_on_updates {
            use tauri_plugin_notification::NotificationExt;
            let body = if result.delegated {
                format!("Opened {} \u{2014} update within the app", detail.display_name)
            } else if result.success {
                format!("{} updated successfully", detail.display_name)
            } else {
                format!("Failed to update {}", detail.display_name)
            };

            let mut builder = app_handle.notification().builder().title("macPlus").body(&body);
            if settings.notification_sound {
                builder = builder.sound("Glass");
            }
            let _ = builder.show();
        }
    }

    // Refresh installed_version and clear available update if successful.
    // Skip for delegated updates — the update stays in the list until the
    // next check cycle verifies the version actually changed.
    if result.success && !result.delegated {
        let new_version = crate::detection::bundle_reader::read_bundle(
            std::path::Path::new(&detail.app_path),
        )
        .and_then(|b| b.installed_version)
        .or_else(|| detail.available_update.as_ref().map(|u| u.available_version.clone()));

        let db_guard = db.lock().await;
        if let Some(ref ver) = new_version {
            let _ = db_guard.update_installed_version(detail.id, ver);
        }
        let _ = db_guard.clear_available_updates(detail.id);
        if let Some(ref token) = detail.homebrew_cask_token {
            let _ = db_guard.clear_updates_for_cask_token(token);
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn execute_bulk_update(
    bundle_ids: Vec<String>,
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<UpdateResult>, AppError> {
    let db = db.inner().clone();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));

    // Pre-authenticate with sudo if 2+ apps may need elevation.
    // This shows a single password dialog instead of one per app.
    let needs_elevation_count = {
        let db_guard = db.lock().await;
        bundle_ids.iter().filter(|bid| {
            if let Ok(detail) = db_guard.get_app_detail(bid) {
                may_need_elevation(&detail)
            } else {
                false
            }
        }).count()
    };

    let keepalive_handle = if needs_elevation_count >= 2 {
        let authed = tokio::task::spawn_blocking(sudo_session::pre_authenticate)
            .await
            .unwrap_or(false);

        if authed {
            // Spawn a keepalive task that refreshes the sudo timestamp every 4 minutes
            let stop = Arc::new(AtomicBool::new(false));
            let stop_clone = stop.clone();
            let handle = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(240)).await;
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    let _ = tokio::task::spawn_blocking(sudo_session::refresh_timestamp).await;
                }
            });
            Some((handle, stop))
        } else {
            None
        }
    } else {
        None
    };

    let mut handles = Vec::new();

    for bundle_id in bundle_ids {
        let db = db.clone();
        let app_handle = app_handle.clone();
        let semaphore = semaphore.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let db_guard = db.lock().await;
            let detail = match db_guard.get_app_detail(&bundle_id) {
                Ok(d) => d,
                Err(e) => {
                    return UpdateResult {
                        bundle_id: bundle_id.clone(),
                        success: false,
                        message: Some(format!("Failed to get app detail: {}", e)),
                        source_type: "unknown".to_string(),
                        from_version: None,
                        to_version: None,
                        handled_relaunch: false,
                        delegated: false,
                    };
                }
            };

            // Record history start
            let to_version_raw = detail.available_update.as_ref()
                .map(|u| u.available_version.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let to_version = truncate_version(&to_version_raw).to_string();
            let history_id = db_guard.record_update_start(
                detail.id,
                detail.installed_version.as_deref().unwrap_or("unknown"),
                &to_version,
                &detail.install_source,
            ).ok();
            drop(db_guard);

            let emit_handle = app_handle.clone();
            let bid = bundle_id.clone();

            let on_progress = move |percent: u8, phase: &str, bytes: Option<(u64, Option<u64>)>| {
                let _ = emit_handle.emit(
                    "update-execute-progress",
                    UpdateExecuteProgress {
                        bundle_id: bid.clone(),
                        phase: phase.to_string(),
                        percent,
                        downloaded_bytes: bytes.map(|(d, _)| d),
                        total_bytes: bytes.and_then(|(_, t)| t),
                    },
                );
            };

            let result = match route_and_execute(&detail, &bundle_id, &on_progress).await {
                Ok(r) => {
                    // Record history result
                    if let Some(hid) = history_id {
                        let db_guard = db.lock().await;
                        record_update_result(&db_guard, hid, &r);
                    }

                    let needs_relaunch = r.success
                        && !r.handled_relaunch
                        && (r.source_type == "homebrew_cask" || r.source_type == "homebrew_formula")
                        && app_lifecycle::is_app_running(&bundle_id);

                    let _ = app_handle.emit(
                        "update-execute-complete",
                        UpdateExecuteComplete {
                            bundle_id: bundle_id.clone(),
                            display_name: detail.display_name.clone(),
                            success: r.success,
                            message: r.message.clone(),
                            needs_relaunch,
                            app_path: if needs_relaunch { Some(detail.app_path.clone()) } else { None },
                            delegated: r.delegated,
                        },
                    );
                    if r.success && !r.delegated {
                        let new_version = crate::detection::bundle_reader::read_bundle(
                            std::path::Path::new(&detail.app_path),
                        )
                        .and_then(|b| b.installed_version)
                        .or_else(|| detail.available_update.as_ref().map(|u| u.available_version.clone()));

                        let db_guard = db.lock().await;
                        if let Some(ref ver) = new_version {
                            let _ = db_guard.update_installed_version(detail.id, ver);
                        }
                        let _ = db_guard.clear_available_updates(detail.id);
                        if let Some(ref token) = detail.homebrew_cask_token {
                            let _ = db_guard.clear_updates_for_cask_token(token);
                        }
                    }
                    r
                }
                Err(e) => {
                    // Record history failure
                    if let Some(hid) = history_id {
                        let db_guard = db.lock().await;
                        let _ = db_guard.record_update_failed(hid, &e.to_string());
                    }

                    let source = AppSource::from_str(&detail.install_source);
                    let _ = app_handle.emit(
                        "update-execute-complete",
                        UpdateExecuteComplete {
                            bundle_id: bundle_id.clone(),
                            display_name: detail.display_name.clone(),
                            success: false,
                            message: Some(e.to_string()),
                            needs_relaunch: false,
                            app_path: None,
                            delegated: false,
                        },
                    );
                    UpdateResult {
                        bundle_id: bundle_id.clone(),
                        success: false,
                        message: Some(e.to_string()),
                        source_type: source.as_str().to_string(),
                        from_version: detail.installed_version.clone(),
                        to_version: None,
                        handled_relaunch: false,
                        delegated: false,
                    }
                }
            };

            result
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    // Cancel the sudo keepalive task now that all updates are done
    if let Some((handle, stop)) = keepalive_handle {
        stop.store(true, Ordering::Relaxed);
        handle.abort();
        let _ = handle.await;
    }

    Ok(results)
}

/// Check whether an app's update path is likely to need elevation.
fn may_need_elevation(detail: &AppDetail) -> bool {
    // Check the update source_type first
    if let Some(ref update) = detail.available_update {
        match update.source_type.as_str() {
            "homebrew_cask" | "sparkle" | "github" | "homebrew_api" | "microsoft_autoupdate" => return true,
            "mas" => return true,
            "adobe_cc" => return false,
            _ => {}
        }
    }
    // Fall back to install_source
    match AppSource::from_str(&detail.install_source) {
        AppSource::Homebrew | AppSource::HomebrewFormula => true,
        _ => false,
    }
}

#[tauri::command]
pub async fn relaunch_app(
    bundle_id: String,
    app_path: String,
) -> Result<(), AppError> {
    // Quit the old version gracefully
    let app_name = std::path::Path::new(&app_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("App")
        .to_string();

    app_lifecycle::quit_app_gracefully(&app_name, &bundle_id);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Launch the new version in background
    app_lifecycle::relaunch_app(&app_path);
    Ok(())
}
