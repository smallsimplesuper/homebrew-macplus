use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::{AppDetail, AppSummary, AvailableUpdateInfo, UpdateSourceInfo};
use crate::scheduler;
use crate::utils::AppError;

#[tauri::command]
pub async fn get_all_apps(
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<AppSummary>, AppError> {
    let db = db.lock().await;
    db.get_all_apps()
}

#[tauri::command]
pub async fn get_app_detail(
    bundle_id: String,
    app_handle: tauri::AppHandle,
) -> Result<AppDetail, AppError> {
    // Bypass the shared mutex — WAL mode allows concurrent readers.
    // Open a short-lived read-only connection so we never block on long-running
    // background operations (scan, update check, cask token backfill).
    let db_path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Custom(e.to_string()))?
        .join("macplus.db");

    tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| AppError::Custom(format!("open read conn: {e}")))?;

        let app = conn
            .query_row(
                "SELECT id, bundle_id, display_name, app_path, installed_version, bundle_version,
                        icon_cache_path, architectures, install_source, obtained_from,
                        homebrew_cask_token, is_ignored, first_seen_at, last_seen_at, mas_app_id,
                        homebrew_formula_name
                 FROM apps WHERE bundle_id = ?1",
                [&bundle_id],
                |row| {
                    let arch_json: Option<String> = row.get(7)?;
                    Ok(AppDetail {
                        id: row.get(0)?,
                        bundle_id: row.get(1)?,
                        display_name: row.get(2)?,
                        app_path: row.get(3)?,
                        installed_version: row.get(4)?,
                        bundle_version: row.get(5)?,
                        icon_cache_path: row.get(6)?,
                        architectures: arch_json.and_then(|j| serde_json::from_str(&j).ok()),
                        install_source: row.get(8)?,
                        obtained_from: row.get(9)?,
                        homebrew_cask_token: row.get(10)?,
                        is_ignored: row.get::<_, i32>(11)? != 0,
                        first_seen_at: row.get(12)?,
                        last_seen_at: row.get(13)?,
                        mas_app_id: row.get(14)?,
                        homebrew_formula_name: row.get(15)?,
                        update_sources: Vec::new(),
                        available_update: None,
                    })
                },
            )
            .map_err(|e| AppError::Custom(format!("query app: {e}")))?;

        let mut sources_stmt = conn
            .prepare(
                "SELECT source_type, source_url, is_primary, last_checked_at
                 FROM update_sources WHERE app_id = ?1",
            )
            .map_err(|e| AppError::Custom(format!("prepare sources: {e}")))?;

        let update_sources: Vec<UpdateSourceInfo> = sources_stmt
            .query_map([app.id], |row| {
                Ok(UpdateSourceInfo {
                    source_type: row.get(0)?,
                    source_url: row.get(1)?,
                    is_primary: row.get::<_, i32>(2)? != 0,
                    last_checked_at: row.get(3)?,
                })
            })
            .map_err(|e| AppError::Custom(format!("query sources: {e}")))?
            .filter_map(|r| r.ok())
            .collect();

        let available_update: Option<AvailableUpdateInfo> = conn
            .query_row(
                "SELECT available_version, source_type, release_notes_url, download_url,
                        release_notes, is_paid_upgrade, detected_at, notes
                 FROM available_updates
                 WHERE app_id = ?1 AND dismissed_at IS NULL
                 ORDER BY detected_at DESC LIMIT 1",
                [app.id],
                |row| {
                    Ok(AvailableUpdateInfo {
                        available_version: row.get(0)?,
                        source_type: row.get(1)?,
                        release_notes_url: row.get(2)?,
                        download_url: row.get(3)?,
                        release_notes: row.get(4)?,
                        is_paid_upgrade: row.get::<_, i32>(5)? != 0,
                        detected_at: row.get(6)?,
                        notes: row.get(7)?,
                    })
                },
            )
            .ok();

        Ok(AppDetail {
            update_sources,
            available_update,
            ..app
        })
    })
    .await
    .map_err(|e| AppError::Custom(e.to_string()))?
}

#[tauri::command]
pub async fn trigger_full_scan(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<usize, AppError> {
    let db = db.inner().clone();
    scheduler::run_full_scan(&app_handle, &db).await
}

#[tauri::command]
pub async fn set_app_ignored(
    bundle_id: String,
    ignored: bool,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), AppError> {
    let db = db.lock().await;
    db.set_app_ignored(&bundle_id, ignored)
}
