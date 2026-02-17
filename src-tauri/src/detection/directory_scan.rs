use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};

use super::bundle_reader;
use super::AppDetector;
use crate::models::DetectedApp;
use crate::utils::AppResult;

pub struct DirectoryScanDetector {
    extra_locations: Vec<String>,
    scan_depth: u32,
}

impl DirectoryScanDetector {
    pub fn new(extra_locations: Vec<String>, scan_depth: u32) -> Self {
        Self { extra_locations, scan_depth }
    }
}

impl Default for DirectoryScanDetector {
    fn default() -> Self {
        Self {
            extra_locations: Vec::new(),
            scan_depth: 2,
        }
    }
}

fn scan_directory(dir: &Path, max_depth: u32) -> Vec<PathBuf> {
    scan_directory_recursive(dir, 0, max_depth)
}

fn scan_directory_recursive(dir: &Path, current_depth: u32, max_depth: u32) -> Vec<PathBuf> {
    let mut apps = Vec::new();
    if current_depth > max_depth {
        return apps;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "app") {
                apps.push(path);
            } else if path.is_dir() && current_depth < max_depth {
                // Skip hidden directories and .app bundles (which are directories internally)
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && !name.ends_with(".app") {
                    apps.extend(scan_directory_recursive(&path, current_depth + 1, max_depth));
                }
            }
        }
    }
    apps
}

/// Discover `/Volumes/*/Applications/` directories on mounted volumes.
fn discover_volume_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let volumes = PathBuf::from("/Volumes");
    if let Ok(entries) = fs::read_dir(&volumes) {
        for entry in entries.flatten() {
            let apps_dir = entry.path().join("Applications");
            if apps_dir.is_dir() {
                dirs.push(apps_dir);
            }
        }
    }
    dirs
}

/// Expand `~` prefix to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[async_trait]
impl AppDetector for DirectoryScanDetector {
    fn name(&self) -> &str {
        "Directory Scan"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        let mut dirs = vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ];

        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Applications"));
        }

        // Add user-configured extra locations
        for loc in &self.extra_locations {
            let expanded = expand_tilde(loc);
            if expanded.is_dir() && !dirs.contains(&expanded) {
                dirs.push(expanded);
            }
        }

        // Auto-discover Applications dirs on mounted volumes
        for vol_dir in discover_volume_app_dirs() {
            if !dirs.contains(&vol_dir) {
                log::info!("Auto-discovered volume app dir: {}", vol_dir.display());
                dirs.push(vol_dir);
            }
        }

        let mut apps = Vec::new();
        for dir in &dirs {
            for app_path in scan_directory(dir, self.scan_depth) {
                if let Some(bundle) = bundle_reader::read_bundle(&app_path) {
                    let source = bundle_reader::detect_install_source(&app_path);
                    apps.push(DetectedApp {
                        bundle_id: bundle.bundle_id,
                        display_name: bundle.display_name,
                        app_path: bundle.app_path,
                        installed_version: bundle.installed_version,
                        bundle_version: bundle.bundle_version,
                        install_source: source,
                        obtained_from: None,
                        homebrew_cask_token: None,
                        architectures: bundle.architectures,
                        sparkle_feed_url: bundle.sparkle_feed_url,
                        mas_app_id: None,
                    homebrew_formula_name: None,
                    });
                }
            }
        }

        Ok(apps)
    }
}
