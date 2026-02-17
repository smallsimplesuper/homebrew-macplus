use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter};

pub fn start_fs_watcher(app_handle: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to create fs watcher: {}", e);
                return;
            }
        };

        let dirs = ["/Applications"];
        for dir in &dirs {
            if Path::new(dir).exists() {
                if let Err(e) = watcher.watch(Path::new(dir), RecursiveMode::NonRecursive) {
                    log::warn!("Failed to watch {}: {}", dir, e);
                }
            }
        }

        if let Some(home) = dirs::home_dir() {
            let user_apps = home.join("Applications");
            if user_apps.exists() {
                let _ = watcher.watch(&user_apps, RecursiveMode::NonRecursive);
            }
        }

        log::info!("FSEvents watcher started for /Applications");

        for result in rx {
            match result {
                Ok(event) => match event.kind {
                    EventKind::Create(_) => {
                        for path in &event.paths {
                            if path.extension().map_or(false, |e| e == "app") {
                                log::info!("New app detected: {:?}", path);
                                let _ = app_handle.emit("app-installed", path.to_string_lossy().to_string());
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in &event.paths {
                            if path.extension().map_or(false, |e| e == "app") {
                                log::info!("App removed: {:?}", path);
                                let _ = app_handle.emit("app-removed", path.to_string_lossy().to_string());
                            }
                        }
                    }
                    _ => {}
                },
                Err(e) => log::warn!("FS watch error: {:?}", e),
            }
        }
    });
}
