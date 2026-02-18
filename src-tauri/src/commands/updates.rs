use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::scheduler;
use crate::utils::AppError;

#[tauri::command]
pub async fn check_all_updates(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    http_client: State<'_, reqwest::Client>,
) -> Result<usize, AppError> {
    let db = db.inner().clone();
    let client = http_client.inner().clone();
    scheduler::run_update_check(&app_handle, &db, &client).await
}

#[tauri::command]
pub async fn check_single_update(
    bundle_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
    http_client: State<'_, reqwest::Client>,
) -> Result<Option<crate::models::UpdateInfo>, AppError> {
    let db_guard = db.lock().await;
    let apps = db_guard.get_all_apps()?;
    drop(db_guard);

    let app = apps
        .into_iter()
        .find(|a| a.bundle_id == bundle_id)
        .ok_or_else(|| AppError::NotFound(format!("App not found: {}", bundle_id)))?;

    let install_source = crate::models::AppSource::from_str(&app.install_source);
    let dispatcher = crate::updaters::UpdateDispatcher::new();

    // Fetch cask index for single-app checks too (enables HomebrewApi checker)
    let cask_index = crate::updaters::homebrew_api::fetch_cask_index(http_client.inner())
        .await
        .map(std::sync::Arc::new);

    // Load GitHub mapping for this specific app
    let github_repo = {
        let db_guard = db.lock().await;
        let mappings = db_guard.get_github_mappings();
        mappings.get(&bundle_id).cloned()
    };

    let context = crate::updaters::AppCheckContext {
        homebrew_cask_token: app.homebrew_cask_token.clone(),
        sparkle_feed_url: app.sparkle_feed_url.clone(),
        obtained_from: app.obtained_from.clone(),
        brew_outdated: None,
        brew_outdated_formulae: None,
        homebrew_cask_index: cask_index,
        github_repo,
        homebrew_formula_name: app.homebrew_formula_name.clone(),
        xcode_clt_installed: None,
        db: Some(db.inner().clone()),
    };

    let result = dispatcher
        .check_update(
            &app.bundle_id,
            &app.app_path,
            app.installed_version.as_deref(),
            &install_source,
            http_client.inner(),
            &context,
        )
        .await?;

    if let Some(ref update) = result {
        let db_guard = db.lock().await;
        let _ = db_guard.upsert_available_update(app.id, update);
    }

    Ok(result)
}

#[tauri::command]
pub async fn debug_update_check(
    bundle_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
    http_client: State<'_, reqwest::Client>,
) -> Result<crate::updaters::UpdateCheckDiagnostic, AppError> {
    let db_guard = db.lock().await;
    let apps = db_guard.get_all_apps()?;
    drop(db_guard);

    let app = apps
        .into_iter()
        .find(|a| a.bundle_id == bundle_id)
        .ok_or_else(|| AppError::NotFound(format!("App not found: {}", bundle_id)))?;

    let install_source = crate::models::AppSource::from_str(&app.install_source);
    let dispatcher = crate::updaters::UpdateDispatcher::new();

    let cask_index = crate::updaters::homebrew_api::fetch_cask_index(http_client.inner())
        .await
        .map(std::sync::Arc::new);

    let github_repo = {
        let db_guard = db.lock().await;
        let mappings = db_guard.get_github_mappings();
        mappings.get(&bundle_id).cloned()
    };

    let context = crate::updaters::AppCheckContext {
        homebrew_cask_token: app.homebrew_cask_token.clone(),
        sparkle_feed_url: app.sparkle_feed_url.clone(),
        obtained_from: app.obtained_from.clone(),
        brew_outdated: None,
        brew_outdated_formulae: None,
        homebrew_cask_index: cask_index,
        github_repo,
        homebrew_formula_name: app.homebrew_formula_name.clone(),
        xcode_clt_installed: None,
        db: Some(db.inner().clone()),
    };

    let checkers_tried = dispatcher
        .debug_check(
            &app.bundle_id,
            &app.app_path,
            app.installed_version.as_deref(),
            &install_source,
            http_client.inner(),
            &context,
        )
        .await;

    Ok(crate::updaters::UpdateCheckDiagnostic {
        bundle_id: app.bundle_id.clone(),
        app_path: app.app_path.clone(),
        installed_version: app.installed_version.clone(),
        install_source: app.install_source.clone(),
        homebrew_cask_token: app.homebrew_cask_token.clone(),
        checkers_tried,
    })
}

#[tauri::command]
pub async fn get_update_count(
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<usize, AppError> {
    let db = db.lock().await;
    db.get_update_count()
}

#[tauri::command]
pub async fn get_update_history(
    limit: Option<i64>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<crate::models::UpdateHistoryEntry>, AppError> {
    // Bypass the shared mutex entirely — WAL mode allows concurrent readers.
    // Open a short-lived read-only connection so we never block on long-running
    // background operations (scan, update check, cask token backfill).
    let db_path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Custom(e.to_string()))?
        .join("macplus.db");
    let limit = limit.unwrap_or(50);

    tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| AppError::Custom(format!("open read conn: {e}")))?;

        let mut stmt = conn
            .prepare(
                "SELECT h.id, a.bundle_id, a.display_name, a.icon_cache_path,
                        h.from_version, h.to_version, h.source_type,
                        h.status, h.error_message, h.started_at, h.completed_at
                 FROM update_history h
                 JOIN apps a ON a.id = h.app_id
                 ORDER BY h.started_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| AppError::Custom(format!("prepare: {e}")))?;

        let entries = stmt
            .query_map([limit], |row| {
                Ok(crate::models::UpdateHistoryEntry {
                    id: row.get(0)?,
                    bundle_id: row.get(1)?,
                    display_name: row.get(2)?,
                    icon_cache_path: row.get(3)?,
                    from_version: row.get(4)?,
                    to_version: row.get(5)?,
                    source_type: row.get(6)?,
                    status: row.get(7)?,
                    error_message: row.get(8)?,
                    started_at: row.get(9)?,
                    completed_at: row.get(10)?,
                })
            })
            .map_err(|e| AppError::Custom(format!("query: {e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    })
    .await
    .map_err(|e| AppError::Custom(e.to_string()))?
}
