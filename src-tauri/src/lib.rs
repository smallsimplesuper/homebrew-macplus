pub mod commands;
pub mod db;
pub mod detection;
pub mod executor;
pub mod models;
pub mod platform;
pub mod scheduler;
pub mod updaters;
pub mod utils;

use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition,
};
use tokio::sync::Mutex;

use db::Database;
use utils::http_client;

/// Managed state holding the tray "update count" menu item for runtime text updates.
pub struct UpdateCountMenuItem(pub tauri::menu::MenuItem<tauri::Wry>);

/// Position the window centered below the given tray icon rectangle.
fn position_window_below_tray(window: &tauri::WebviewWindow, tray_rect: &tauri::Rect) {
    let scale = window.scale_factor().unwrap_or(1.0);
    // Window width in logical pixels (from tauri.conf.json) → physical
    let win_width = 640.0 * scale;

    // Convert tray rect position/size to physical pixels
    let tray_pos = tray_rect.position.to_physical::<i32>(scale);
    let tray_size = tray_rect.size.to_physical::<u32>(scale);

    // Tray icon center X (physical pixels)
    let tray_center_x = tray_pos.x as f64 + tray_size.width as f64 / 2.0;
    let window_x = tray_center_x - win_width / 2.0;
    let window_y = tray_pos.y as f64 + tray_size.height as f64;

    // Clamp to screen bounds
    if let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    {
        let screen_pos = monitor.position();
        let screen_size = monitor.size();
        let screen_right = screen_pos.x as f64 + screen_size.width as f64;
        let clamped_x = window_x.max(screen_pos.x as f64).min(screen_right - win_width);
        let _ = window.set_position(PhysicalPosition::new(clamped_x as i32, window_y as i32));
    } else {
        let _ = window.set_position(PhysicalPosition::new(window_x as i32, window_y as i32));
    }
}

/// Toggle the main window: show+focus if hidden/unfocused, hide if visible+focused.
/// Positions the window below the tray icon when showing.
fn toggle_main_window(app: &tauri::AppHandle, tray_rect: tauri::Rect) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);
        if is_visible && is_focused {
            let _ = window.hide();
        } else {
            position_window_below_tray(&window, &tray_rect);
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Show the main window below the tray icon (always shows, never toggles).
fn show_main_window_below_tray(app: &tauri::AppHandle, tray_rect: &tauri::Rect) {
    if let Some(window) = app.get_webview_window("main") {
        position_window_below_tray(&window, tray_rect);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_store::Builder::default().build())
        // .plugin(tauri_plugin_updater::Builder::new().build()) // TODO: enable when pubkey is configured
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::apps::get_all_apps,
            commands::apps::get_app_detail,
            commands::apps::trigger_full_scan,
            commands::apps::set_app_ignored,
            commands::updates::check_all_updates,
            commands::updates::check_single_update,
            commands::updates::debug_update_check,
            commands::updates::get_update_count,
            commands::updates::get_update_history,
            commands::execute::execute_update,
            commands::execute::execute_bulk_update,
            commands::execute::relaunch_app,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::check_paths_exist,
            commands::system::open_app,
            commands::system::reveal_in_finder,
            commands::system::get_app_icon,
            commands::system::get_permissions_status,
            commands::system::get_permissions_passive,
            commands::system::trigger_automation_permission,
            commands::system::open_system_preferences,
            commands::system::check_setup_status,
            commands::system::ensure_askpass_helper,
            commands::system::open_terminal_with_command,
            commands::system::check_connectivity,
            commands::self_update::check_self_update,
            commands::self_update::execute_self_update,
            commands::self_update::relaunch_self,
            commands::uninstall::uninstall_app,
            commands::uninstall::scan_associated_files,
        ])
        // Part 2: Hide main window on close instead of quitting
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // Initialize database
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("macplus.db");
            let database = Database::new(&db_path)
                .expect("Failed to initialize database");
            let db = Arc::new(Mutex::new(database));
            app.manage(db.clone());

            // Initialize askpass helper
            if let Ok(resource_dir) = app.path().resource_dir() {
                crate::utils::askpass::init_askpass_path(resource_dir);
            }

            // Clean up stale self-update artifacts from previous runs
            {
                let backup = std::path::Path::new("/Applications/macPlus.app.update-backup");
                if backup.exists() {
                    let _ = std::fs::remove_dir_all(backup);
                }
                if let Ok(entries) = std::fs::read_dir("/tmp") {
                    for entry in entries.flatten().take(200) {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        if name.starts_with("macplus-update-") || name.starts_with("macplus-self-update-") {
                            let _ = std::fs::remove_dir_all(entry.path());
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }

            // Add icon cache directory to asset protocol scope
            if let Ok(cache_dir) = app.path().app_cache_dir() {
                let icons_dir = cache_dir.join("icons");
                let _ = std::fs::create_dir_all(&icons_dir);
                let _ = app.asset_protocol_scope().allow_directory(&icons_dir, true);
            }

            // Validate settings — prune stale scan locations from migrated databases
            {
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    scheduler::validate_settings(&db_clone).await;
                });
            }

            // Initialize HTTP client
            let client = http_client::create_http_client();
            app.manage(client.clone());

            // Apply vibrancy to main window
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                    let _ = apply_vibrancy(&window, NSVisualEffectMaterial::Sidebar, None, Some(10.0));
                }
                let _ = window;
            }

            // Read check interval from settings
            let check_interval = {
                let db_guard = db.blocking_lock();
                scheduler::load_settings_interval(&db_guard)
            };

            // Setup system tray
            let check_now = MenuItemBuilder::with_id("check_now", "Check for Updates")
                .build(app)?;
            let update_count_item = MenuItemBuilder::with_id("update_count", "No updates available")
                .enabled(false)
                .build(app)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let open_app = MenuItemBuilder::with_id("open_app", "Open macPlus")
                .build(app)?;
            let separator2 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit macPlus")
                .build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&check_now, &update_count_item, &separator, &open_app, &separator2, &quit])
                .build()?;

            app.manage(UpdateCountMenuItem(update_count_item));

            let tray_icon_path = app.path().resolve(
                "icons/tray-icon.png",
                tauri::path::BaseDirectory::Resource,
            )?;
            let tray_icon_bytes = std::fs::read(&tray_icon_path)?;
            let tray_icon = tauri::image::Image::from_bytes(&tray_icon_bytes)?.to_owned();

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("macPlus")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "check_now" => {
                        let _ = app.emit("trigger-check", ());
                        if let Some(tray) = app.tray_by_id("main-tray") {
                            if let Ok(Some(rect)) = tray.rect() {
                                show_main_window_below_tray(app, &rect);
                                return;
                            }
                        }
                        // Fallback: show without positioning
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "open_app" => {
                        if let Some(tray) = app.tray_by_id("main-tray") {
                            if let Ok(Some(rect)) = tray.rect() {
                                show_main_window_below_tray(app, &rect);
                                return;
                            }
                        }
                        // Fallback: show without positioning
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            rect,
                            ..
                        } => {
                            toggle_main_window(tray.app_handle(), rect);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Start FSEvents watcher
            scheduler::fs_watcher::start_fs_watcher(app.handle().clone());

            // Start periodic update checks using the configured interval
            scheduler::start_periodic_checks(
                app.handle().clone(),
                db,
                client,
                check_interval,
            );

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building macPlus")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    // Prevent Cmd+Q from quitting — hide to tray instead.
                    // The tray menu "Quit macPlus" uses app.exit(0) which bypasses this.
                    api.prevent_exit();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
                tauri::RunEvent::Reopen { has_visible_windows, .. } => {
                    if !has_visible_windows {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                _ => {}
            }
        });
}
