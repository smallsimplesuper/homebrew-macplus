use std::collections::HashMap;

use crate::models::{AppSource, DetectedApp};

pub fn deduplicate(apps: Vec<DetectedApp>) -> Vec<DetectedApp> {
    let mut by_bundle_id: HashMap<String, DetectedApp> = HashMap::new();

    for app in apps {
        if app.bundle_id.is_empty() {
            continue;
        }

        match by_bundle_id.get_mut(&app.bundle_id) {
            Some(existing) => {
                merge_into(existing, &app);
            }
            None => {
                by_bundle_id.insert(app.bundle_id.clone(), app);
            }
        }
    }

    let mut result: Vec<DetectedApp> = by_bundle_id.into_values().collect();
    result.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    result
}

fn merge_into(existing: &mut DetectedApp, new: &DetectedApp) {
    // Prefer non-empty display name
    if existing.display_name.is_empty() && !new.display_name.is_empty() {
        existing.display_name = new.display_name.clone();
    }

    // Prefer non-empty app path
    if existing.app_path.is_empty() && !new.app_path.is_empty() {
        existing.app_path = new.app_path.clone();
    }

    // Prefer version from bundle reader (most accurate)
    if existing.installed_version.is_none() {
        existing.installed_version = new.installed_version.clone();
    }

    if existing.bundle_version.is_none() {
        existing.bundle_version = new.bundle_version.clone();
    }

    // Upgrade install source from Unknown
    if existing.install_source == AppSource::Unknown && new.install_source != AppSource::Unknown {
        existing.install_source = new.install_source.clone();
    }

    // Homebrew source overrides direct for brew-installed apps
    if new.install_source == AppSource::Homebrew {
        existing.install_source = AppSource::Homebrew;
        existing.homebrew_cask_token = new.homebrew_cask_token.clone();
    }

    // MAS overrides other sources
    if new.install_source == AppSource::MacAppStore {
        existing.install_source = AppSource::MacAppStore;
    }

    // Merge optional metadata
    if existing.obtained_from.is_none() {
        existing.obtained_from = new.obtained_from.clone();
    }

    if existing.homebrew_cask_token.is_none() {
        existing.homebrew_cask_token = new.homebrew_cask_token.clone();
    }

    if existing.architectures.is_none() {
        existing.architectures = new.architectures.clone();
    }

    if existing.sparkle_feed_url.is_none() {
        existing.sparkle_feed_url = new.sparkle_feed_url.clone();
    }

    if existing.homebrew_formula_name.is_none() {
        existing.homebrew_formula_name = new.homebrew_formula_name.clone();
    }
}
