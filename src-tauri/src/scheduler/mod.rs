pub mod fs_watcher;
pub mod scan_scheduler;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use futures::stream::{self, StreamExt};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::detection::DetectionEngine;
use crate::models::{AppSettings, AppSource, ScanComplete, ScanProgress, UpdateCheckComplete, UpdateFound};
use crate::platform::icon_extractor;
use crate::updaters::homebrew_api::{self, HomebrewCaskIndex};
use crate::updaters::homebrew_cask::{fetch_brew_outdated, fetch_brew_outdated_formulae};
use crate::updaters::{AppCheckContext, BrewOutdatedCask, BrewOutdatedFormula, UpdateDispatcher};
use crate::utils::{is_browser_extension, is_xcode_clt_installed, AppResult};

/// Load the check interval (in minutes) from settings for use at startup.
pub fn load_settings_interval(db: &crate::db::Database) -> u64 {
    load_settings_from_db(db).check_interval_minutes as u64
}

pub fn load_settings_from_db(db: &crate::db::Database) -> AppSettings {
    let json: Option<String> = db
        .conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'app_settings'",
            [],
            |row| row.get(0),
        )
        .ok();

    match json {
        Some(j) => serde_json::from_str(&j).unwrap_or_default(),
        None => AppSettings::default(),
    }
}

pub async fn run_full_scan(
    app_handle: &AppHandle,
    db: &Arc<Mutex<Database>>,
) -> AppResult<usize> {
    let start = std::time::Instant::now();

    let (scan_locations, scan_depth) = {
        let db_guard = db.lock().await;
        let settings = load_settings_from_db(&db_guard);
        (settings.scan_locations, settings.scan_depth)
    };

    let engine = DetectionEngine::with_scan_locations(scan_locations, scan_depth);

    let handle = app_handle.clone();
    let apps = engine
        .detect_all(|phase, current, total| {
            let _ = handle.emit(
                "scan-progress",
                ScanProgress {
                    phase: phase.to_string(),
                    current,
                    total,
                    app_name: None,
                },
            );
        })
        .await?;

    let count = apps.len();
    {
        let db_guard = db.lock().await;
        let _ = db_guard.conn.execute_batch("BEGIN");
        for app in &apps {
            let _ = db_guard.upsert_app(app);
        }
        let _ = db_guard.conn.execute_batch("COMMIT");

        // Extract icons for non-formula apps
        if let Ok(cache_dir) = app_handle.path().app_cache_dir() {
            let icons_dir = cache_dir.join("icons");
            if std::fs::create_dir_all(&icons_dir).is_ok() {
                // First pass: update DB for apps that already have cached icons
                let mut apps_needing_icons: Vec<(String, String)> = Vec::new();
                for app in &apps {
                    if app.install_source == AppSource::HomebrewFormula {
                        continue;
                    }

                    let expected_path = icons_dir.join(format!("{}.png", app.bundle_id));
                    if expected_path.exists() {
                        let path_str = expected_path.to_string_lossy().to_string();
                        let _ = db_guard.update_icon_cache_path(&app.bundle_id, &path_str);
                    } else {
                        apps_needing_icons.push((app.bundle_id.clone(), app.app_path.clone()));
                    }
                }
                drop(db_guard);

                let apps_needing_icons_count = apps_needing_icons.len();
                // Extract icons in parallel (up to 8 concurrent tasks)
                let icons_dir = Arc::new(icons_dir);
                let icon_results: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

                stream::iter(apps_needing_icons)
                    .for_each_concurrent(8, |(bundle_id, app_path)| {
                        let icons_dir = icons_dir.clone();
                        let icon_results = icon_results.clone();
                        async move {
                            let app_path = std::path::Path::new(&app_path).to_path_buf();
                            let icons_dir_inner = icons_dir.clone();
                            let task = tokio::task::spawn_blocking(move || {
                                icon_extractor::extract_icon_png(&app_path, &icons_dir_inner)
                            });
                            let result = tokio::time::timeout(Duration::from_secs(10), task).await;

                            match result {
                                Ok(Ok(Ok(Some(icon_path)))) => {
                                    icon_results.lock().await.push((bundle_id, icon_path));
                                }
                                Ok(Ok(Ok(None))) => {
                                    log::debug!("No icon found for {}", bundle_id);
                                }
                                Ok(Ok(Err(e))) => {
                                    log::debug!("Icon extraction failed for {}: {}", bundle_id, e);
                                }
                                Ok(Err(e)) => {
                                    log::debug!("Icon extraction task panicked for {}: {}", bundle_id, e);
                                }
                                Err(_) => {
                                    log::debug!("Icon extraction timed out for {}", bundle_id);
                                }
                            }
                        }
                    })
                    .await;

                // Batch-update icon paths in DB
                let results = icon_results.lock().await;
                let extracted = results.len();
                log::info!("Icon extraction: {}/{} icons extracted successfully", extracted, apps_needing_icons_count);
                if !results.is_empty() {
                    let db_guard = db.lock().await;
                    let _ = db_guard.conn.execute_batch("BEGIN");
                    for (bundle_id, icon_path) in results.iter() {
                        let _ = db_guard.update_icon_cache_path(bundle_id, icon_path);
                    }
                    let _ = db_guard.conn.execute_batch("COMMIT");
                }
            }
        }
    }

    let _ = app_handle.emit(
        "scan-complete",
        ScanComplete {
            app_count: count,
            duration_ms: start.elapsed().as_millis() as u64,
        },
    );

    Ok(count)
}

pub async fn run_update_check(
    app_handle: &AppHandle,
    db: &Arc<Mutex<Database>>,
    http_client: &reqwest::Client,
) -> AppResult<usize> {
    let start = std::time::Instant::now();
    let dispatcher = Arc::new(UpdateDispatcher::new());

    // Reset GitHub rate-limit flag for this cycle
    crate::updaters::github_releases::reset_rate_limit_flag();

    let apps = {
        let db = db.lock().await;
        db.get_all_apps()?
    };

    let total = apps.len();
    let checked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let updates_found = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Pre-compute brew outdated, formulae, and cask index concurrently
    let http_for_index = http_client.clone();
    let (brew_outdated_res, brew_outdated_formulae_res, cask_index_res) = tokio::join!(
        tokio::task::spawn_blocking(fetch_brew_outdated),
        tokio::task::spawn_blocking(fetch_brew_outdated_formulae),
        homebrew_api::fetch_cask_index(&http_for_index),
    );

    let brew_outdated: Arc<HashMap<String, BrewOutdatedCask>> =
        Arc::new(brew_outdated_res.unwrap_or_default());
    log::info!("brew outdated found {} outdated casks", brew_outdated.len());

    let brew_outdated_formulae: Arc<HashMap<String, BrewOutdatedFormula>> =
        Arc::new(brew_outdated_formulae_res.unwrap_or_default());
    log::info!("brew outdated found {} outdated formulae", brew_outdated_formulae.len());

    // Check Xcode CLT once for the entire cycle (only relevant when formulae are outdated)
    let xcode_clt_installed: Option<bool> = if !brew_outdated_formulae.is_empty() {
        Some(tokio::task::spawn_blocking(is_xcode_clt_installed).await.unwrap_or(true))
    } else {
        None
    };

    let cask_index: Option<Arc<HomebrewCaskIndex>> = cask_index_res.map(Arc::new);

    // Backfill cask tokens for apps that match the index but lack a token
    if let Some(ref index) = cask_index {
        backfill_cask_tokens(db, index).await;
    }

    // Load GitHub repo mappings from database once for all apps
    let github_mappings: HashMap<String, String> = {
        let db_guard = db.lock().await;
        db_guard.get_github_mappings()
    };

    let github_mappings = Arc::new(github_mappings);

    let check_apps: Vec<_> = apps
        .iter()
        .filter(|app| !app.is_ignored)
        .collect();

    stream::iter(check_apps)
        .for_each_concurrent(10, |app| {
            let dispatcher = dispatcher.clone();
            let app_handle = app_handle.clone();
            let db = db.clone();
            let http_client = http_client.clone();
            let checked = checked.clone();
            let updates_found = updates_found.clone();
            let brew_outdated = brew_outdated.clone();
            let brew_outdated_formulae = brew_outdated_formulae.clone();
            let cask_index = cask_index.clone();
            let github_mappings = github_mappings.clone();
            let xcode_clt_installed = xcode_clt_installed;

            async move {
                let count = checked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let _ = app_handle.emit(
                    "update-check-progress",
                    crate::models::UpdateCheckProgress {
                        checked: count,
                        total,
                        current_app: Some(app.display_name.clone()),
                    },
                );

                let install_source = crate::models::AppSource::from_str(&app.install_source);
                let context = AppCheckContext {
                    homebrew_cask_token: app.homebrew_cask_token.clone(),
                    sparkle_feed_url: app.sparkle_feed_url.clone(),
                    obtained_from: app.obtained_from.clone(),
                    brew_outdated: Some(brew_outdated.clone()),
                    brew_outdated_formulae: Some(brew_outdated_formulae.clone()),
                    homebrew_cask_index: cask_index.clone(),
                    github_repo: github_mappings.get(&app.bundle_id).cloned()
                        .or_else(|| cask_index.as_ref()
                            .and_then(|idx| idx.github_repos.get(&app.bundle_id.to_lowercase()).cloned())),
                    homebrew_formula_name: app.homebrew_formula_name.clone(),
                    xcode_clt_installed,
                    db: Some(db.clone()),
                };

                if let Ok(Some(update)) = dispatcher
                    .check_update(
                        &app.bundle_id,
                        &app.app_path,
                        app.installed_version.as_deref(),
                        &install_source,
                        &http_client,
                        &context,
                    )
                    .await
                {
                    let dominated = app.installed_version.as_ref()
                        .map(|iv| update.available_version == *iv)
                        .unwrap_or(false);

                    if dominated {
                        log::info!(
                            "Skipping no-op update for {}: available '{}' == installed",
                            app.bundle_id, update.available_version,
                        );
                    } else {
                        let _ = app_handle.emit(
                            "update-found",
                            UpdateFound {
                                bundle_id: app.bundle_id.clone(),
                                current_version: app.installed_version.clone(),
                                available_version: update.available_version.clone(),
                                source: update.source_type.as_str().to_string(),
                            },
                        );

                        let db = db.lock().await;
                        let _ = db.upsert_available_update(app.id, &update);
                        updates_found.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        })
        .await;

    // Persist GitHub ETag cache to disk
    crate::updaters::github_releases::save_etag_cache().await;

    // Check for macPlus self-update and emit event if available
    if let Some(info) = crate::commands::self_update::check_self_update_inner(http_client).await {
        let _ = app_handle.emit("self-update-available", &info);
    }

    let found_this_cycle = updates_found.load(std::sync::atomic::Ordering::Relaxed);
    log::info!("Update check found {} new updates this cycle", found_this_cycle);

    // Use the total DB count so the emitted value matches what the UI displays
    let db_count = {
        let db_guard = db.lock().await;
        db_guard.get_update_count().unwrap_or(found_this_cycle)
    };

    let _ = app_handle.emit(
        "update-check-complete",
        UpdateCheckComplete {
            updates_found: db_count,
            duration_ms: start.elapsed().as_millis() as u64,
        },
    );

    // Load settings for notification + tray updates
    let settings = {
        let db_guard = db.lock().await;
        load_settings_from_db(&db_guard)
    };

    // Send native notification if updates were found and notifications are enabled
    if found_this_cycle > 0 && settings.notification_on_updates {
        use tauri_plugin_notification::NotificationExt;
        let body = if found_this_cycle == 1 {
            "1 app update available".to_string()
        } else {
            format!("{} app updates available", found_this_cycle)
        };
        let mut builder = app_handle
            .notification()
            .builder()
            .title("macPlus")
            .body(&body);
        if settings.notification_sound {
            builder = builder.sound("default");
        }
        let _ = builder.show();
    }

    // Update tray tooltip, icon, and menu item with update count
    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        let tooltip = if settings.show_badge_count && db_count > 0 {
            format!("macPlus — {} update{}", db_count, if db_count == 1 { "" } else { "s" })
        } else {
            "macPlus".to_string()
        };
        let _ = tray.set_tooltip(Some(&tooltip));

        // Swap tray icon based on update availability
        let icon_path = if db_count > 0 {
            app_handle.path().resolve("icons/tray-icon-update.png", tauri::path::BaseDirectory::Resource)
        } else {
            app_handle.path().resolve("icons/tray-icon.png", tauri::path::BaseDirectory::Resource)
        };
        if let Ok(path) = icon_path {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(icon) = tauri::image::Image::from_bytes(&bytes) {
                    let _ = tray.set_icon(Some(icon.to_owned()));
                }
            }
        }
    }

    // Update the tray menu item text
    {
        let state = app_handle.state::<crate::UpdateCountMenuItem>();
        let text = if db_count > 0 {
            format!("{} update{} available", db_count, if db_count == 1 { "" } else { "s" })
        } else {
            "No updates available".to_string()
        };
        let _ = state.0.set_text(&text);
    }

    Ok(db_count)
}

/// Backfill cask tokens for apps that match the Homebrew API index
/// but currently have no `homebrew_cask_token` set. This enables
/// `brew upgrade --cask <token>` for directly-installed apps.
async fn backfill_cask_tokens(
    db: &Arc<Mutex<Database>>,
    index: &HomebrewCaskIndex,
) {
    let db_guard = db.lock().await;
    let apps = match db_guard.get_all_apps() {
        Ok(a) => a,
        Err(e) => {
            log::warn!("Failed to load apps for cask token backfill: {}", e);
            return;
        }
    };

    let mut backfilled = 0usize;
    for app in &apps {
        if app.homebrew_cask_token.is_some() {
            continue;
        }

        // Browser extensions must not be matched to Homebrew casks
        if is_browser_extension(&app.bundle_id) {
            continue;
        }

        let app_path = std::path::Path::new(&app.app_path);
        if let Some(token) = index.lookup_token(&app.bundle_id, app_path) {
            if let Err(e) = db_guard.update_cask_token(&app.bundle_id, token) {
                log::info!("Failed to backfill cask token for {}: {}", app.bundle_id, e);
            } else {
                backfilled += 1;
                log::info!(
                    "Backfilled cask token '{}' for {}",
                    token,
                    app.bundle_id
                );
            }
        }
    }

    if backfilled > 0 {
        log::info!("Backfilled cask tokens for {} apps", backfilled);
    }
}

pub fn start_periodic_checks(
    app_handle: AppHandle,
    db: Arc<Mutex<Database>>,
    http_client: reqwest::Client,
    initial_interval_minutes: u64,
) {
    tauri::async_runtime::spawn(async move {
        let mut interval_mins = initial_interval_minutes;

        loop {
            tokio::time::sleep(Duration::from_secs(interval_mins * 60)).await;

            log::info!("Running periodic update check...");
            match run_update_check(&app_handle, &db, &http_client).await {
                Ok(count) => log::info!("Periodic check found {} updates", count),
                Err(e) => log::warn!("Periodic check failed: {}", e),
            }

            // Re-read interval from settings for the next cycle (hot-reload)
            let new_interval = {
                let db_guard = db.lock().await;
                load_settings_interval(&db_guard)
            };
            if new_interval != interval_mins {
                log::info!(
                    "Check interval changed: {} min -> {} min",
                    interval_mins, new_interval
                );
                interval_mins = new_interval;
            }
        }
    });
}
