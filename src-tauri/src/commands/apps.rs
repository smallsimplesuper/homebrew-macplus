use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::{AppDetail, AppSummary};
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
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<AppDetail, AppError> {
    let db = db.lock().await;
    db.get_app_detail(&bundle_id)
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
