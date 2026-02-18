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

    // Emit initial progress event immediately so the UI shows activity right away
    let _ = app_handle.emit(
        "scan-progress",
        ScanProgress {
            phase: "Starting".to_string(),
            current: 0,
            total: 6,
            app_name: None,
        },
    );

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

        // Emit progress: extracting icons phase
        let _ = app_handle.emit(
            "scan-progress",
            ScanProgress {
                phase: "Finalising apps".to_string(),
                current: 6,
                total: 6,
                app_name: None,
            },
        );

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
                // Extract icons in parallel (up to 16 concurrent tasks)
                let icons_dir = Arc::new(icons_dir);
                let icon_results: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

                stream::iter(apps_needing_icons)
                    .for_each_concurrent(16, |(bundle_id, app_path)| {
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

    // Emit progress: indexing phase (cask token backfill)
    let _ = app_handle.emit(
        "scan-progress",
        ScanProgress {
            phase: "Indexing".to_string(),
            current: 6,
            total: 6,
            app_name: None,
        },
    );

    // Backfill cask tokens for newly discovered apps
    let client = app_handle.state::<reqwest::Client>();
    if let Some(index) = homebrew_api::fetch_cask_index(client.inner()).await {
        backfill_cask_tokens(db, &Arc::new(index)).await;
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

/// Validate settings on startup: remove non-existent scan locations
/// (except /Volumes/ paths which may be temporarily unmounted).
pub async fn validate_settings(db: &Arc<Mutex<Database>>) {
    let db_guard = db.lock().await;
    let settings = load_settings_from_db(&db_guard);
    drop(db_guard);

    let mut pruned = Vec::new();
    let mut removed = Vec::new();
    for loc in &settings.scan_locations {
        let expanded = std::path::Path::new(loc);
        if expanded.exists() {
            pruned.push(loc.clone());
        } else if loc.starts_with("/Volumes/") {
            // Keep unmounted volume paths — drive might be temporarily disconnected
            log::warn!("Settings: scan location '{}' not found (keeping — may be unmounted volume)", loc);
            pruned.push(loc.clone());
        } else {
            log::warn!("Settings: removing stale scan location '{}' (path does not exist)", loc);
            removed.push(loc.clone());
        }
    }

    if !removed.is_empty() {
        let mut updated = settings.clone();
        // If all locations were pruned, reset to defaults
        if pruned.is_empty() {
            updated.scan_locations = vec!["/Applications".to_string(), "~/Applications".to_string()];
            log::info!("Settings: all scan locations were stale — reset to defaults");
        } else {
            updated.scan_locations = pruned;
        }

        let json = match serde_json::to_string(&updated) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("Settings: failed to serialize pruned settings: {}", e);
                return;
            }
        };

        let db_guard = db.lock().await;
        let _ = db_guard.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('app_settings', ?1, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            [&json],
        );
        log::info!("Settings: removed {} stale scan locations", removed.len());
    }
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

    // Emit initial progress event immediately
    let _ = app_handle.emit(
        "update-check-progress",
        crate::models::UpdateCheckProgress {
            checked: 0,
            total,
            current_app: Some("Preparing...".to_string()),
        },
    );

    // Emit progress: fetching Homebrew data
    let _ = app_handle.emit(
        "update-check-progress",
        crate::models::UpdateCheckProgress {
            checked: 0,
            total,
            current_app: Some("Fetching Homebrew data...".to_string()),
        },
    );

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

        // Backfill descriptions from the cask index
        let db_guard = db.lock().await;
        if let Ok(apps) = db_guard.get_apps_missing_descriptions() {
            let mut desc_count = 0usize;
            for (app_id, cask_token, _, _) in &apps {
                if let Some(token) = cask_token {
                    if let Some(desc) = index.lookup_desc(token) {
                        let _ = db_guard.update_description(*app_id, desc);
                        desc_count += 1;
                    }
                }
            }
            if desc_count > 0 {
                log::info!("Backfilled descriptions for {} apps", desc_count);
            }
        }
        drop(db_guard);
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

    let updated_app_ids: Arc<Mutex<std::collections::HashSet<i64>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let successfully_checked_ids: Arc<Mutex<std::collections::HashSet<i64>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));

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
            let updated_app_ids = updated_app_ids.clone();
            let successfully_checked_ids = successfully_checked_ids.clone();

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

                match dispatcher
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
                    Ok(Some(update)) => {
                        successfully_checked_ids.lock().await.insert(app.id);

                        let dominated = {
                            let db_match = app.installed_version.as_ref()
                                .map(|iv| update.available_version == *iv)
                                .unwrap_or(false);
                            let fresh_match = update.current_version.as_ref()
                                .map(|cv| update.available_version == *cv)
                                .unwrap_or(false);
                            db_match || fresh_match
                        };

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

                            {
                                let db = db.lock().await;
                                let _ = db.upsert_available_update(app.id, &update);
                            }
                            updates_found.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            updated_app_ids.lock().await.insert(app.id);
                        }
                    }
                    Ok(None) => {
                        successfully_checked_ids.lock().await.insert(app.id);
                    }
                    Err(e) => {
                        log::debug!("Checker error for {}: {}", app.bundle_id, e);
                    }
                }
            }
        })
        .await;

    // Persist GitHub ETag cache to disk (timeout so slow I/O doesn't block completion)
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        crate::updaters::github_releases::save_etag_cache(),
    ).await;

    // Check for macPlus self-update and emit event if available
    crate::updaters::github_releases::reset_rate_limit_flag();
    if let Some(info) = crate::commands::self_update::check_self_update_inner(http_client).await {
        let _ = app_handle.emit("self-update-available", &info);
    }

    let found_this_cycle = updates_found.load(std::sync::atomic::Ordering::Relaxed);
    log::info!("Update check found {} new updates this cycle", found_this_cycle);

    // --- Post-cycle stale update cleanup ---
    {
        let updated_ids = updated_app_ids.lock().await;
        let db_guard = db.lock().await;

        // Step 1: Refresh installed_version from disk for apps with pending updates.
        // This ensures the version-match purge works even when the DB version is stale
        // (e.g., user updated an app via MAS between scans).
        if let Ok(mut stmt) = db_guard.conn.prepare(
            "SELECT DISTINCT a.id, a.app_path FROM apps a
             JOIN available_updates au ON au.app_id = a.id
             WHERE au.dismissed_at IS NULL"
        ) {
            let candidates: Vec<(i64, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap_or_else(|_| unreachable!())
                .filter_map(|r| r.ok())
                .collect();

            for (app_id, app_path) in &candidates {
                if let Some(bundle) = crate::detection::bundle_reader::read_bundle(
                    std::path::Path::new(app_path),
                ) {
                    if let Some(ref ver) = bundle.installed_version {
                        let _ = db_guard.update_installed_version(*app_id, ver);
                    }
                }
            }
        }

        // Step 2: Purge updates where available_version now matches the (freshly updated)
        // installed_version. Also matches comma-containing Homebrew versions where the
        // numeric prefix equals the installed version (e.g. "1.1.3363,abc..." == "1.1.3363").
        let purged = db_guard.conn.execute(
            "DELETE FROM available_updates WHERE id IN (
                SELECT au.id FROM available_updates au
                JOIN apps a ON a.id = au.app_id
                WHERE au.dismissed_at IS NULL
                  AND (au.available_version = a.installed_version
                       OR (au.available_version LIKE a.installed_version || ',%'))
            )",
            [],
        ).unwrap_or(0);

        // Step 3: Clear remaining stale updates for apps that were successfully checked
        // this cycle but received no update. Apps whose checkers errored are excluded
        // so a network glitch doesn't silently clear a valid pending update.
        let checked_ids = successfully_checked_ids.lock().await;
        let mut cleared = 0usize;
        for app_id in checked_ids.iter() {
            if !updated_ids.contains(app_id) {
                cleared += db_guard.conn.execute(
                    "DELETE FROM available_updates WHERE app_id = ?1 AND dismissed_at IS NULL",
                    [app_id],
                ).unwrap_or(0);
            }
        }

        if purged > 0 || cleared > 0 {
            log::info!(
                "Post-cycle cleanup: {} version-matched purged, {} stale cleared",
                purged, cleared
            );
        }
    }

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
            builder = builder.sound("Glass");
        }
        match builder.show() {
            Ok(_) => log::info!("Sent native notification: {} updates", found_this_cycle),
            Err(e) => log::warn!("Failed to send notification: {}", e),
        }
    }

    // Update tray tooltip, icon, and menu item with update count
    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        let tooltip = if settings.show_badge_count && db_count > 0 {
            format!("macPlus — {} update{}", db_count, if db_count == 1 { "" } else { "s" })
        } else {
            "macPlus".to_string()
        };
        let _ = tray.set_tooltip(Some(&tooltip));

        // Render tray icon — with numbered badge if enabled and updates available
        let base_icon_path = app_handle.path().resolve("icons/tray-icon.png", tauri::path::BaseDirectory::Resource);
        if let Ok(path) = base_icon_path {
            if let Ok(base_bytes) = std::fs::read(&path) {
                let icon_bytes = if settings.show_badge_count && db_count > 0 {
                    crate::platform::tray_badge::render_tray_icon_with_badge(&base_bytes, db_count)
                        .unwrap_or_else(|| base_bytes.clone())
                } else if db_count > 0 {
                    // Fallback: use static update icon when badge count is disabled
                    let update_path = app_handle.path().resolve("icons/tray-icon-update.png", tauri::path::BaseDirectory::Resource);
                    update_path.ok().and_then(|p| std::fs::read(p).ok()).unwrap_or(base_bytes.clone())
                } else {
                    base_bytes.clone()
                };
                if let Ok(icon) = tauri::image::Image::from_bytes(&icon_bytes) {
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
