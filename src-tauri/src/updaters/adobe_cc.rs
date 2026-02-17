use async_trait::async_trait;
use std::path::Path;

use super::cask_sha_checker::{self, CaskShaResult};
use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::AppResult;

const ADOBE_BUNDLE_IDS: &[&str] = &[
    "com.adobe.Photoshop",
    "com.adobe.Illustrator",
    "com.adobe.InDesign",
    "com.adobe.Lightroom",
    "com.adobe.LightroomClassicCC",
    "com.adobe.PremierePro",
    "com.adobe.AfterEffects",
    "com.adobe.Acrobat.Pro",
    "com.adobe.Reader",
    "com.adobe.AdobeMediaEncoder",
    "com.adobe.Audition",
    "com.adobe.Animate",
    "com.adobe.Dreamweaver",
    "com.adobe.bridge",
    "com.adobe.dimension",
    "com.adobe.substance.3d-painter",
    "com.adobe.InCopy",
    "com.adobe.Character.Animator",
    "com.adobe.Fresco",
    "com.adobe.XD",
];

/// Hardcoded bundle_id → cask_token mapping for common Adobe apps.
/// Used when `context.homebrew_cask_token` is None and `index.lookup_token()` fails.
const ADOBE_CASK_TOKENS: &[(&str, &str)] = &[
    ("com.adobe.Photoshop", "adobe-photoshop"),
    ("com.adobe.Illustrator", "adobe-illustrator"),
    ("com.adobe.InDesign", "adobe-indesign"),
    ("com.adobe.PremierePro", "adobe-premiere-pro"),
    ("com.adobe.AfterEffects", "adobe-after-effects"),
    ("com.adobe.Lightroom", "adobe-lightroom"),
    ("com.adobe.LightroomClassicCC", "adobe-lightroom-classic"),
    ("com.adobe.Acrobat.Pro", "adobe-acrobat-pro"),
    ("com.adobe.Reader", "adobe-acrobat-reader"),
    ("com.adobe.AdobeMediaEncoder", "adobe-media-encoder"),
    ("com.adobe.Audition", "adobe-audition"),
    ("com.adobe.Animate", "adobe-animate"),
    ("com.adobe.Dreamweaver", "adobe-dreamweaver"),
    ("com.adobe.bridge", "adobe-bridge"),
    ("com.adobe.dimension", "adobe-dimension"),
];

pub struct AdobeCCChecker;

impl AdobeCCChecker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UpdateChecker for AdobeCCChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::AdobeCc
    }

    fn can_check(&self, bundle_id: &str, _app_path: &Path, _install_source: &AppSource) -> bool {
        ADOBE_BUNDLE_IDS.iter().any(|&id| bundle_id.eq_ignore_ascii_case(id))
            || (bundle_id.starts_with("com.adobe.") && is_creative_tool(bundle_id))
    }

    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        // Diagnostic summary of available detection methods
        let home = dirs::home_dir();
        let cc_cache_exists = home.as_ref().map_or(false, |h| {
            h.join("Library/Application Support/Adobe").is_dir()
        });
        let rum_exists = Path::new("/usr/local/bin/RemoteUpdateManager").exists();
        let has_brew_index = context.homebrew_cask_index.is_some();
        let has_cask_token = context.homebrew_cask_token.is_some()
            || (has_brew_index && context.homebrew_cask_index.as_ref()
                .and_then(|idx| idx.lookup_token(bundle_id, app_path)).is_some())
            || lookup_hardcoded_token(bundle_id).is_some();
        let sap_code = bundle_to_sap_code(bundle_id);

        log::info!(
            "Adobe CC: checking {} (path: {}) — diagnostics: cc_cache_dir_exists={}, rum_exists={}, brew_index={}, cask_token={}, sap_code={:?}",
            bundle_id, app_path.display(), cc_cache_exists, rum_exists, has_brew_index, has_cask_token, sap_code
        );

        let current = match current_version {
            Some(v) => v.to_string(),
            None => {
                log::info!("Adobe CC: no current version for {}, skipping", bundle_id);
                return Ok(None);
            }
        };

        // Prefer the precise version from Adobe's application.xml if it has more
        // segments than the Info.plist version (e.g. "26.5.0" vs "26.5").
        let current = match read_adobe_application_xml(app_path) {
            Some(xml_ver) => {
                let xml_segments = xml_ver.split('.').count();
                let plist_segments = current.split('.').count();
                if xml_segments > plist_segments {
                    log::info!(
                        "Adobe CC: using application.xml version {} over Info.plist version {} for {}",
                        xml_ver, current, bundle_id
                    );
                    xml_ver
                } else {
                    current
                }
            }
            None => current,
        };

        // 1) Check CC Desktop's local update cache for available versions
        if let Some(update) = check_cc_update_cache(bundle_id, &current, app_path) {
            return Ok(Some(update));
        }

        // 2) Try RUM (Remote Update Manager) if available
        if let Some(update) = check_rum_updates(bundle_id, &current).await {
            return Ok(Some(update));
        }

        // 3) Try Homebrew cask index
        log::warn!("Adobe CC: {} — CC cache and RUM found no update, trying Homebrew fallback", bundle_id);
        let index = match &context.homebrew_cask_index {
            Some(idx) => idx,
            None => {
                log::warn!("Adobe CC: no Homebrew cask index available for {} — all detection methods exhausted with no update found", bundle_id);
                // Try SHA check as last resort
                return self.try_sha_fallback(bundle_id, &current, app_path, client, context).await;
            }
        };

        let cask_info = match index.lookup(bundle_id, app_path) {
            Some(info) => info,
            None => {
                log::info!(
                    "Adobe CC: index lookup returned None for {} (path: {}). \
                     Adobe casks use version \"latest\" -- checking brew outdated --greedy.",
                    bundle_id,
                    app_path.display()
                );

                // Fallback: check brew outdated --greedy via cask token
                if let Some(ref outdated_map) = context.brew_outdated {
                    let cask_token = context.homebrew_cask_token.as_deref()
                        .or_else(|| index.lookup_token(bundle_id, app_path))
                        .or_else(|| lookup_hardcoded_token(bundle_id));

                    if let Some(token) = cask_token {
                        if let Some(outdated) = outdated_map.get(token) {
                            log::info!(
                                "Adobe CC: {} found in brew outdated via token '{}' (installed: {}, available: {})",
                                bundle_id, token, outdated.installed_versions, outdated.current_version
                            );
                            return Ok(Some(UpdateInfo {
                                bundle_id: bundle_id.to_string(),
                                current_version: Some(current.to_string()),
                                available_version: outdated.current_version.clone(),
                                source_type: UpdateSourceType::AdobeCc,
                                download_url: None,
                                release_notes_url: None,
                                release_notes: None,
                                is_paid_upgrade: false,
                                notes: Some("Update available via Homebrew".to_string()),
                            }));
                        }
                    }
                }

                // Try SHA-256 change detection as final fallback
                return self.try_sha_fallback(bundle_id, &current, app_path, client, context).await;
            }
        };

        // If this app is from a known Homebrew cask and the cask is NOT in
        // brew outdated, it's up to date.
        if context.homebrew_cask_token.is_some() {
            if let Some(ref outdated_map) = context.brew_outdated {
                if !outdated_map.contains_key(&cask_info.token) {
                    return Ok(None);
                }
            }
        }

        if version_compare::is_newer(&current, &cask_info.version) {
            log::info!(
                "Adobe CC: {} has update {} -> {} (cask: {})",
                bundle_id, current, cask_info.version, cask_info.token
            );
            return Ok(Some(UpdateInfo {
                bundle_id: bundle_id.to_string(),
                current_version: Some(current.to_string()),
                available_version: cask_info.version.clone(),
                source_type: UpdateSourceType::AdobeCc,
                download_url: None,
                release_notes_url: None,
                release_notes: None,
                is_paid_upgrade: false,
                notes: None,
            }));
        }

        log::warn!(
            "Adobe CC: {} — no update detected across all methods (current version: {})",
            bundle_id, current
        );
        Ok(None)
    }
}

impl AdobeCCChecker {
    /// Try SHA-256 change detection for "latest" casks as a final fallback.
    async fn try_sha_fallback(
        &self,
        bundle_id: &str,
        current: &str,
        app_path: &Path,
        client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        let cask_token = context.homebrew_cask_token.as_deref()
            .or_else(|| context.homebrew_cask_index.as_ref()
                .and_then(|idx| idx.lookup_token(bundle_id, app_path)))
            .or_else(|| lookup_hardcoded_token(bundle_id));

        let Some(token) = cask_token else {
            log::info!(
                "Adobe CC: no cask token found for {} — update detection limited",
                bundle_id
            );
            return Ok(None);
        };

        let Some(ref db) = context.db else {
            log::info!("Adobe CC: no DB handle for SHA check of {}", bundle_id);
            return Ok(None);
        };

        match cask_sha_checker::check_cask_sha(token, client, db).await {
            CaskShaResult::Changed => {
                Ok(Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: Some(current.to_string()),
                    available_version: format!("{} (newer build)", current),
                    source_type: UpdateSourceType::AdobeCc,
                    download_url: None,
                    release_notes_url: None,
                    release_notes: None,
                    is_paid_upgrade: false,
                    notes: Some("Update detected via cask SHA change — reinstall via Homebrew or Creative Cloud".to_string()),
                }))
            }
            CaskShaResult::NoCheck => {
                log::info!("Adobe CC: {} uses sha256 :no_check — cannot detect updates", token);
                Ok(None)
            }
            CaskShaResult::Unchanged | CaskShaResult::FirstSeen => Ok(None),
            CaskShaResult::Error(e) => {
                log::info!("Adobe CC: SHA check error for {}: {}", token, e);
                Ok(None)
            }
        }
    }
}

/// Look up a hardcoded cask token for an Adobe bundle ID.
fn lookup_hardcoded_token(bundle_id: &str) -> Option<&'static str> {
    ADOBE_CASK_TOKENS
        .iter()
        .find(|(bid, _)| bid.eq_ignore_ascii_case(bundle_id))
        .map(|(_, token)| *token)
}

/// Read the precise installed version from Adobe's local application.xml.
/// Each Adobe app embeds version info at {app_path}/Contents/Resources/application.xml.
pub fn read_adobe_application_xml(app_path: &Path) -> Option<String> {
    let xml_path = app_path.join("Contents/Resources/application.xml");
    let content = std::fs::read_to_string(&xml_path).ok()?;

    // Parse MajorVersion, MinorVersion, PatchVersion from the XML
    let major = extract_xml_element(&content, "MajorVersion")?;
    let minor = extract_xml_element(&content, "MinorVersion").unwrap_or_else(|| "0".to_string());
    let patch = extract_xml_element(&content, "PatchVersion").unwrap_or_else(|| "0".to_string());

    Some(format!("{}.{}.{}", major, minor, patch))
}

/// Simple XML element extraction (avoids full XML parser for this small use case).
fn extract_xml_element(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);

    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;

    let value = xml[start..end].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Map Adobe bundle IDs to SAP codes used in CC Desktop's update cache.
fn bundle_to_sap_code(bundle_id: &str) -> Option<&str> {
    match bundle_id {
        "com.adobe.Photoshop" => Some("PHSP"),
        "com.adobe.Illustrator" => Some("ILST"),
        "com.adobe.InDesign" => Some("IDSN"),
        "com.adobe.PremierePro" => Some("PPRO"),
        "com.adobe.AfterEffects" => Some("AEFT"),
        "com.adobe.Lightroom" => Some("LTRM"),
        "com.adobe.LightroomClassicCC" => Some("LRCC"),
        "com.adobe.Acrobat.Pro" => Some("APRO"),
        "com.adobe.AdobeMediaEncoder" => Some("AME"),
        "com.adobe.Audition" => Some("AUDT"),
        "com.adobe.Animate" => Some("FLPR"),
        "com.adobe.bridge" => Some("KBRG"),
        "com.adobe.Dreamweaver" => Some("DRWV"),
        "com.adobe.dimension" => Some("ESHR"),
        "com.adobe.Reader" => Some("ARDR"),
        "com.adobe.substance.3d-painter" => Some("SBSTP"),
        "com.adobe.InCopy" => Some("AICY"),
        "com.adobe.Character.Animator" => Some("CHAR"),
        "com.adobe.Fresco" => Some("FRSC"),
        "com.adobe.XD" => Some("SPRK"),
        _ => None,
    }
}

/// Dynamically walk ~/Library/Application Support/Adobe/ looking for
/// any updater-data/v1/products directory (handles various CC versions).
fn cc_cache_dirs() -> Vec<std::path::PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let mut dirs = Vec::new();

    // Known paths (including newer CC Desktop locations)
    let known = [
        "Library/Application Support/Adobe/OOBE/PDApp/UWA/UpdaterCore/updater-data/v1/products",
        "Library/Application Support/Adobe/Adobe Desktop Common/RemoteComponents/UPI/UnifiedPlugin/updater-data/v1/products",
        "Library/Application Support/Adobe/Adobe Desktop Common/HDBox/updater-data/v1/products",
        "Library/Application Support/Adobe/ACCC/HDBox/updater-data/v1/products",
    ];
    for p in &known {
        let full = home.join(p);
        let exists = full.exists();
        log::info!("Adobe CC: cache path check: {} (exists={})", full.display(), exists);
        if !dirs.contains(&full) {
            dirs.push(full);
        }
    }

    // Recursive walk: search for updater-data/v1/products under Adobe support dir (max depth 6)
    let adobe_support = home.join("Library/Application Support/Adobe");
    if adobe_support.is_dir() {
        let skip_names = [".", "Logs", "CoreSync"];
        recursive_find_products_dir(&adobe_support, 0, 6, &skip_names, &mut dirs);
    }

    log::info!("Adobe CC: discovered {} cache directories: {:?}",
        dirs.len(),
        dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>()
    );

    dirs
}

/// Recursively search for directories ending in `updater-data/v1/products`.
fn recursive_find_products_dir(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    skip_names: &[&str],
    found: &mut Vec<std::path::PathBuf>,
) {
    if depth > max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        if skip_names.iter().any(|&s| name == s) {
            continue;
        }

        // Check if this directory IS "products" and its parent path ends with updater-data/v1
        if name == "products" {
            if let Some(parent) = path.parent() {
                if parent.ends_with("updater-data/v1") && !found.contains(&path) {
                    found.push(path.clone());
                    continue;
                }
            }
        }

        // Recurse deeper
        recursive_find_products_dir(&path, depth + 1, max_depth, skip_names, found);
    }
}

/// Check if a product ID from the CC cache matches the given bundle ID,
/// using suffix matching, SAP code lookup, and prefix matching.
fn product_matches_bundle(product_id: &str, bundle_id: &str) -> bool {
    // Direct suffix match: com.adobe.Photoshop → Photoshop
    if let Some(bid_suffix) = bundle_id.strip_prefix("com.adobe.") {
        if product_id.eq_ignore_ascii_case(bid_suffix) {
            return true;
        }

        // Prefix match: product ID "Photoshop2025" should match bundle suffix "Photoshop"
        if product_id.len() > bid_suffix.len() {
            let prefix = &product_id[..bid_suffix.len()];
            if prefix.eq_ignore_ascii_case(bid_suffix) {
                return true;
            }
        }
    }

    // SAP code match: com.adobe.Photoshop → PHSP
    if let Some(sap) = bundle_to_sap_code(bundle_id) {
        if product_id.eq_ignore_ascii_case(sap) {
            return true;
        }

        // SAP code prefix match: "PHSP_26" or "PHSP-26.0" should match
        // Require a separator char (_, -, .) after the SAP code
        if product_id.len() > sap.len() {
            let prefix = &product_id[..sap.len()];
            if prefix.eq_ignore_ascii_case(sap) {
                let sep = product_id.as_bytes()[sap.len()];
                if sep == b'_' || sep == b'-' || sep == b'.' {
                    return true;
                }
            }
        }
    }

    false
}

/// Extract product ID and version from a JSON object, trying multiple field patterns.
fn extract_update_fields(data: &serde_json::Value) -> Option<(String, String)> {
    let product_id = data.get("productId").and_then(|v| v.as_str())
        .or_else(|| data.get("sapCode").and_then(|v| v.as_str()))
        .or_else(|| data.get("id").and_then(|v| v.as_str()))
        .or_else(|| data.get("product").and_then(|p| {
            p.get("sapCode").and_then(|v| v.as_str())
                .or_else(|| p.get("id").and_then(|v| v.as_str()))
        }));

    let version = data.get("productVersion").and_then(|v| v.as_str())
        .or_else(|| data.get("version").and_then(|v| v.as_str()))
        .or_else(|| data.get("availableVersion").and_then(|v| v.as_str()))
        .or_else(|| data.get("product").and_then(|p| {
            p.get("version").and_then(|v| v.as_str())
        }));

    match (product_id, version) {
        (Some(pid), Some(ver)) => Some((pid.to_string(), ver.to_string())),
        _ => None,
    }
}

/// JSON filenames to try in each product/version subdirectory.
const CACHE_JSON_FILENAMES: &[&str] = &[
    "application.json",
    "update.json",
    "product.json",
    "manifest.json",
];

/// Check CC Desktop's local update cache for available updates.
/// Adobe Creative Cloud Desktop caches available update info locally.
fn check_cc_update_cache(bundle_id: &str, current_version: &str, _app_path: &Path) -> Option<UpdateInfo> {
    let cache_dirs = cc_cache_dirs();

    let mut any_dir_found = false;
    for cache_dir in &cache_dirs {
        if !cache_dir.exists() {
            log::info!("Adobe CC: cache dir not found: {}", cache_dir.display());
            continue;
        }
        any_dir_found = true;

        let entries = match std::fs::read_dir(cache_dir) {
            Ok(e) => e,
            Err(err) => {
                log::debug!("Adobe CC: failed to read cache dir {}: {}", cache_dir.display(), err);
                continue;
            }
        };

        // Collect and log all product directory names for diagnostics
        let all_entries: Vec<_> = entries.flatten().collect();
        let dir_names: Vec<String> = all_entries.iter()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        log::info!(
            "Adobe CC: product dirs in {}: {:?} (looking for match with {})",
            cache_dir.display(), dir_names, bundle_id
        );

        for entry in all_entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Try JSON files directly in the product directory
            if let Some(update) = try_json_files_in_dir(&path, bundle_id, current_version) {
                return Some(update);
            }

            // Also check one level deeper (nested version subdirectories like PHSP/27.0/)
            if let Ok(sub_entries) = std::fs::read_dir(&path) {
                for sub_entry in sub_entries.flatten() {
                    let sub_path = sub_entry.path();
                    if sub_path.is_dir() {
                        if let Some(update) = try_json_files_in_dir(&sub_path, bundle_id, current_version) {
                            return Some(update);
                        }
                    }
                }
            }
        }
    }

    if !any_dir_found {
        log::info!("Adobe CC: no CC Desktop cache directories found for {}", bundle_id);
    }

    None
}

/// Try all known JSON filenames in a directory and check for update info.
fn try_json_files_in_dir(dir: &Path, bundle_id: &str, current_version: &str) -> Option<UpdateInfo> {
    for &filename in CACHE_JSON_FILENAMES {
        let json_path = dir.join(filename);
        if !json_path.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(&json_path) {
            Ok(c) => c,
            Err(err) => {
                log::debug!("Adobe CC: failed to read {}: {}", json_path.display(), err);
                continue;
            }
        };

        let data: serde_json::Value = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(err) => {
                log::debug!("Adobe CC: failed to parse {}: {}", json_path.display(), err);
                continue;
            }
        };

        // Log the top-level keys for diagnostics
        if let Some(obj) = data.as_object() {
            let keys: Vec<&String> = obj.keys().collect();
            log::info!("Adobe CC: keys in {}: {:?}", json_path.display(), keys);
        }

        let (product_id, available_version) = match extract_update_fields(&data) {
            Some(fields) => fields,
            None => {
                log::debug!(
                    "Adobe CC: {} has no recognizable product ID or version fields",
                    json_path.display()
                );
                continue;
            }
        };

        if !product_matches_bundle(&product_id, bundle_id) {
            continue;
        }

        if version_compare::is_newer(current_version, &available_version) {
            log::info!(
                "Adobe CC: {} has update {} -> {} (from CC Desktop cache, product={}, file={})",
                bundle_id, current_version, available_version, product_id, json_path.display()
            );
            return Some(UpdateInfo {
                bundle_id: bundle_id.to_string(),
                current_version: Some(current_version.to_string()),
                available_version: available_version.to_string(),
                source_type: UpdateSourceType::AdobeCc,
                download_url: None,
                release_notes_url: None,
                release_notes: None,
                is_paid_upgrade: false,
                notes: Some("Update available via Creative Cloud Desktop".to_string()),
            });
        } else {
            log::debug!(
                "Adobe CC: {} matched product {} but version {} is not newer than {}",
                bundle_id, product_id, available_version, current_version
            );
        }
    }

    None
}

/// Check Adobe's Remote Update Manager (RUM) for available updates.
/// RUM is Adobe's CLI tool typically at /usr/local/bin/RemoteUpdateManager.
async fn check_rum_updates(bundle_id: &str, current_version: &str) -> Option<UpdateInfo> {
    let rum_path = Path::new("/usr/local/bin/RemoteUpdateManager");
    if !rum_path.exists() {
        log::debug!("Adobe CC: RUM not found at {}", rum_path.display());
        return None;
    }

    log::info!("Adobe CC: checking RUM for updates for {}", bundle_id);

    let bundle_id_owned = bundle_id.to_string();
    let current_owned = current_version.to_string();

    let result = tokio::task::spawn_blocking(move || {
        run_rum_check(&bundle_id_owned, &current_owned)
    }).await;

    match result {
        Ok(update) => update,
        Err(e) => {
            log::info!("Adobe CC: RUM task panicked: {}", e);
            None
        }
    }
}

/// Run the RUM CLI and parse its output (synchronous, called via spawn_blocking).
fn run_rum_check(bundle_id: &str, current_version: &str) -> Option<UpdateInfo> {
    let output = match std::process::Command::new("/usr/local/bin/RemoteUpdateManager")
        .arg("--action=list")
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::info!("Adobe CC: failed to run RUM: {}", e);
            return None;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        log::info!("Adobe CC: RUM exited with status {} (stderr: {})", output.status, stderr.trim());
        return None;
    }

    log::debug!("Adobe CC: RUM output: {}", stdout.trim());

    // Try JSON parsing first
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(updates) = data.as_array()
            .or_else(|| data.get("updates").and_then(|u| u.as_array()))
        {
            for item in updates {
                if let Some((product_id, available_version)) = extract_update_fields(item) {
                    if product_matches_bundle(&product_id, bundle_id)
                        && version_compare::is_newer(current_version, &available_version)
                    {
                        log::info!(
                            "Adobe CC: {} has update {} -> {} (from RUM, product={})",
                            bundle_id, current_version, available_version, product_id
                        );
                        return Some(UpdateInfo {
                            bundle_id: bundle_id.to_string(),
                            current_version: Some(current_version.to_string()),
                            available_version,
                            source_type: UpdateSourceType::AdobeCc,
                            download_url: None,
                            release_notes_url: None,
                            release_notes: None,
                            is_paid_upgrade: false,
                            notes: Some("Update available (detected via Adobe Remote Update Manager)".to_string()),
                        });
                    }
                }
            }
        }
        return None;
    }

    // Fallback: line-based parsing — look for lines like "SAP_CODE - version - name"
    // or "SAP_CODE/version"
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("Following") {
            continue;
        }

        // Try "SAP_CODE - version - name" format
        let parts: Vec<&str> = line.split(" - ").collect();
        if parts.len() >= 2 {
            let product_id = parts[0].trim();
            let available_version = parts[1].trim();

            if product_matches_bundle(product_id, bundle_id)
                && version_compare::is_newer(current_version, available_version)
            {
                log::info!(
                    "Adobe CC: {} has update {} -> {} (from RUM line, product={})",
                    bundle_id, current_version, available_version, product_id
                );
                return Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: Some(current_version.to_string()),
                    available_version: available_version.to_string(),
                    source_type: UpdateSourceType::AdobeCc,
                    download_url: None,
                    release_notes_url: None,
                    release_notes: None,
                    is_paid_upgrade: false,
                    notes: Some("Update available (detected via Adobe Remote Update Manager)".to_string()),
                });
            }
        }

        // Try "SAP_CODE/version" format
        if let Some((product_id, available_version)) = line.split_once('/') {
            let product_id = product_id.trim();
            let available_version = available_version.trim();

            if product_matches_bundle(product_id, bundle_id)
                && version_compare::is_newer(current_version, available_version)
            {
                log::info!(
                    "Adobe CC: {} has update {} -> {} (from RUM slash format, product={})",
                    bundle_id, current_version, available_version, product_id
                );
                return Some(UpdateInfo {
                    bundle_id: bundle_id.to_string(),
                    current_version: Some(current_version.to_string()),
                    available_version: available_version.to_string(),
                    source_type: UpdateSourceType::AdobeCc,
                    download_url: None,
                    release_notes_url: None,
                    release_notes: None,
                    is_paid_upgrade: false,
                    notes: Some("Update available (detected via Adobe Remote Update Manager)".to_string()),
                });
            }
        }
    }

    None
}

/// Returns true if the bundle ID refers to a creative tool (not the CC Desktop
/// manager or other helper processes).
fn is_creative_tool(bundle_id: &str) -> bool {
    let excluded = [
        "com.adobe.acc",
        "com.adobe.AdobeCreativeCloud",
        "com.adobe.ccx.start",
    ];
    !excluded.iter().any(|&e| bundle_id.eq_ignore_ascii_case(e))
}
