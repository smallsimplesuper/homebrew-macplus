use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::AppSettings;
use crate::utils::AppError;

#[tauri::command]
pub async fn get_settings(
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<AppSettings, AppError> {
    let db = db.lock().await;
    let json: Option<String> = db
        .conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'app_settings'",
            [],
            |row| row.get(0),
        )
        .ok();

    match json {
        Some(j) => serde_json::from_str(&j)
            .map_err(|e| AppError::Custom(format!("Failed to parse settings: {}", e))),
        None => Ok(AppSettings::default()),
    }
}

#[tauri::command]
pub async fn update_settings(
    settings: AppSettings,
    db: State<'_, Arc<Mutex<Database>>>,
    app_handle: tauri::AppHandle,
) -> Result<(), AppError> {
    let json = serde_json::to_string(&settings)
        .map_err(|e| AppError::Custom(format!("Failed to serialize settings: {}", e)))?;

    let update_count = {
        let db = db.lock().await;
        db.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('app_settings', ?1, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            [&json],
        )?;
        db.get_update_count().unwrap_or(0)
    };

    // Apply tray visibility and tooltip
    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        let _ = tray.set_visible(settings.show_menu_bar_icon);

        let tooltip = if settings.show_badge_count && update_count > 0 {
            format!("macPlus — {} update{}", update_count, if update_count == 1 { "" } else { "s" })
        } else {
            "macPlus".to_string()
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }

    Ok(())
}

#[tauri::command]
pub async fn check_paths_exist(
    paths: Vec<String>,
) -> Result<HashMap<String, bool>, AppError> {
    let mut result = HashMap::new();
    for path in paths {
        let expanded = if path.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(path.strip_prefix("~/").unwrap()))
                .unwrap_or_else(|| std::path::PathBuf::from(&path))
        } else {
            std::path::PathBuf::from(&path)
        };
        result.insert(path, expanded.exists());
    }
    Ok(result)
}
