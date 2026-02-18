use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use tokio::sync::RwLock;

use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::{is_browser_extension, AppResult};

struct CaskIndexCache {
    etag: Option<String>,
    index: Option<HomebrewCaskIndex>,
    fetched_at: Option<std::time::Instant>,
}

/// TTL for the cask index cache — skip network requests if the cached index is fresh.
const CASK_INDEX_TTL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60); // 6 hours

fn cask_cache() -> &'static RwLock<CaskIndexCache> {
    static CACHE: OnceLock<RwLock<CaskIndexCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        RwLock::new(CaskIndexCache {
            etag: None,
            index: None,
            fetched_at: None,
        })
    })
}

/// Version info extracted from the Homebrew Formulae API for a single cask.
#[derive(Debug, Clone)]
pub struct CaskVersionInfo {
    pub token: String,
    pub version: String,
    pub url: Option<String>,
    pub sha256: Option<String>,
}

/// Index built from https://formulae.brew.sh/api/cask.json providing fast lookups
/// by bundle ID or app filename.
#[derive(Debug, Clone)]
pub struct HomebrewCaskIndex {
    /// Bundle ID (e.g. "org.mozilla.firefox") → cask info (excludes "latest" versions)
    pub by_bundle_id: HashMap<String, CaskVersionInfo>,
    /// Normalized app filename (e.g. "firefox") → cask info (excludes "latest" versions)
    pub by_app_name: HashMap<String, CaskVersionInfo>,
    /// Bundle ID → cask token for ALL casks including "latest" (for token backfill)
    pub all_tokens_by_bundle_id: HashMap<String, String>,
    /// Normalized app filename → cask token for ALL casks including "latest" (for token backfill)
    pub all_tokens_by_app_name: HashMap<String, String>,
    /// Cask token → download URL (all casks including "latest")
    pub url_by_token: HashMap<String, String>,
    /// Bundle ID → GitHub "owner/repo" slug, auto-extracted from cask download URLs/homepages
    pub github_repos: HashMap<String, String>,
}

/// Normalize an app name for matching: lowercase, strip ".app" suffix.
fn normalize_app_name(name: &str) -> String {
    let s = name.trim();
    let s = if let Some(stripped) = s.strip_suffix(".app") {
        stripped
    } else {
        s
    };
    s.to_lowercase()
}

/// Extract a GitHub "owner/repo" slug from a URL if it matches known patterns.
fn extract_github_slug(url: &str) -> Option<String> {
    // Match: https://github.com/{owner}/{repo}/releases/download/...
    //    or: https://github.com/{owner}/{repo}/archive/...
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.splitn(4, '/').collect();
        if parts.len() >= 3
            && (parts[2] == "releases" || parts[2] == "archive")
            && !parts[0].is_empty()
            && !parts[1].is_empty()
        {
            return Some(format!("{}/{}", parts[0], parts[1]));
        }
    }
    None
}

/// Extract a GitHub "owner/repo" slug from a homepage URL.
fn extract_github_slug_from_homepage(url: &str) -> Option<String> {
    // Match: https://github.com/{owner}/{repo} (exactly 2 path segments)
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let trimmed = rest.trim_end_matches('/');
        let parts: Vec<&str> = trimmed.splitn(3, '/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(format!("{}/{}", parts[0], parts[1]));
        }
    }
    None
}

/// Build the index from parsed JSON cask array.
fn build_index(json: &[serde_json::Value]) -> HomebrewCaskIndex {
    let mut by_bundle_id = HashMap::new();
    let mut by_app_name = HashMap::new();
    let mut all_tokens_by_bundle_id = HashMap::new();
    let mut all_tokens_by_app_name = HashMap::new();
    let mut url_by_token = HashMap::new();
    let mut github_repos: HashMap<String, String> = HashMap::new();

    for cask in json {
        let token = match cask.get("token").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => continue,
        };
        let raw_version = match cask.get("version").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };
        let version = version_compare::strip_brew_version_token(raw_version);

        let url = cask.get("url").and_then(|v| v.as_str()).map(String::from);
        let sha256 = cask.get("sha256").and_then(|v| v.as_str()).map(String::from);

        // Populate url_by_token for all casks (including "latest")
        if let Some(ref u) = url {
            url_by_token.insert(token.to_string(), u.clone());
        }

        let is_latest = version == "latest";

        // Extract GitHub slug from download URL or homepage (skip "latest" casks
        // since we can't do version comparison for them anyway)
        let github_slug = if !is_latest {
            url.as_deref()
                .and_then(extract_github_slug)
                .or_else(|| {
                    cask.get("homepage")
                        .and_then(|v| v.as_str())
                        .and_then(extract_github_slug_from_homepage)
                })
        } else {
            None
        };

        let artifacts = match cask.get("artifacts").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => continue,
        };

        // Only build CaskVersionInfo for non-"latest" casks (used for version comparison)
        let info = if !is_latest {
            Some(CaskVersionInfo {
                token: token.to_string(),
                version: version.to_string(),
                url: url.clone(),
                sha256,
            })
        } else {
            None
        };

        // Collect bundle IDs found for this cask (for GitHub repo association)
        let mut cask_bundle_ids: Vec<String> = Vec::new();

        for artifact in artifacts {
            // Extract app names from artifact "app" arrays
            if let Some(apps) = artifact.get("app").and_then(|v| v.as_array()) {
                for app_entry in apps {
                    if let Some(app_name) = app_entry.as_str() {
                        let normalized = normalize_app_name(app_name);
                        if !normalized.is_empty() {
                            // All-inclusive token map (includes "latest")
                            all_tokens_by_app_name
                                .entry(normalized.clone())
                                .or_insert_with(|| token.to_string());
                            // Version-aware map (excludes "latest")
                            if let Some(ref info) = info {
                                by_app_name
                                    .entry(normalized)
                                    .or_insert_with(|| info.clone());
                            }
                        }
                    }
                }
            }

            // Extract bundle IDs from "uninstall" quit fields
            if let Some(uninstalls) = artifact.get("uninstall").and_then(|v| v.as_array()) {
                for uninstall in uninstalls {
                    if let Some(quits) = uninstall.get("quit").and_then(|v| v.as_array()) {
                        for quit in quits {
                            if let Some(bid) = quit.as_str() {
                                let bid_lower = bid.to_lowercase();
                                all_tokens_by_bundle_id
                                    .entry(bid_lower.clone())
                                    .or_insert_with(|| token.to_string());
                                if let Some(ref info) = info {
                                    by_bundle_id
                                        .entry(bid_lower.clone())
                                        .or_insert_with(|| info.clone());
                                }
                                cask_bundle_ids.push(bid_lower);
                            }
                        }
                    }
                    // Also try "quit" as a single string
                    if let Some(bid) = uninstall.get("quit").and_then(|v| v.as_str()) {
                        let bid_lower = bid.to_lowercase();
                        all_tokens_by_bundle_id
                            .entry(bid_lower.clone())
                            .or_insert_with(|| token.to_string());
                        if let Some(ref info) = info {
                            by_bundle_id
                                .entry(bid_lower.clone())
                                .or_insert_with(|| info.clone());
                        }
                        cask_bundle_ids.push(bid_lower);
                    }
                }
            }

            // Also check "zap" for additional bundle IDs
            if let Some(zaps) = artifact.get("zap").and_then(|v| v.as_array()) {
                for zap in zaps {
                    if let Some(quits) = zap.get("quit").and_then(|v| v.as_array()) {
                        for quit in quits {
                            if let Some(bid) = quit.as_str() {
                                let bid_lower = bid.to_lowercase();
                                all_tokens_by_bundle_id
                                    .entry(bid_lower.clone())
                                    .or_insert_with(|| token.to_string());
                                if let Some(ref info) = info {
                                    by_bundle_id
                                        .entry(bid_lower.clone())
                                        .or_insert_with(|| info.clone());
                                }
                                cask_bundle_ids.push(bid_lower);
                            }
                        }
                    }
                }
            }
        }

        // Associate extracted GitHub slug with all bundle IDs found for this cask
        if let Some(ref slug) = github_slug {
            for bid in &cask_bundle_ids {
                github_repos.entry(bid.clone()).or_insert_with(|| slug.clone());
            }
        }
    }

    log::info!(
        "Homebrew API index: {} casks, {} matched by bundle_id ({} incl. latest), {} matched by app_name ({} incl. latest), {} GitHub repos auto-discovered",
        json.len(),
        by_bundle_id.len(),
        all_tokens_by_bundle_id.len(),
        by_app_name.len(),
        all_tokens_by_app_name.len(),
        github_repos.len(),
    );

    HomebrewCaskIndex {
        by_bundle_id,
        by_app_name,
        all_tokens_by_bundle_id,
        all_tokens_by_app_name,
        url_by_token,
        github_repos,
    }
}

/// Fetches the Homebrew Formulae cask API and builds lookup indexes.
/// Uses ETag caching to avoid re-downloading the full ~1.6MB JSON when unchanged.
pub async fn fetch_cask_index(client: &reqwest::Client) -> Option<HomebrewCaskIndex> {
    let url = "https://formulae.brew.sh/api/cask.json";

    // Return cached index if within TTL — skip the network request entirely
    {
        let cache = cask_cache().read().await;
        if let (Some(ref index), Some(fetched_at)) = (&cache.index, cache.fetched_at) {
            if fetched_at.elapsed() < CASK_INDEX_TTL {
                log::info!("Homebrew cask index cache hit (age: {}s)", fetched_at.elapsed().as_secs());
                return Some(index.clone());
            }
        }
    }

    // Read cached ETag (if any) under a short-lived read lock
    let cached_etag = {
        let cache = cask_cache().read().await;
        cache.etag.clone()
    };

    // Build request with conditional header
    let mut req = client.get(url);
    if let Some(ref etag) = cached_etag {
        req = req.header("If-None-Match", etag.as_str());
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to fetch Homebrew cask index: {}", e);
            // Graceful degradation: return cached index if available
            let cache = cask_cache().read().await;
            return cache.index.clone();
        }
    };

    let status = resp.status();

    // 304 Not Modified — refresh TTL and return cached index
    if status == reqwest::StatusCode::NOT_MODIFIED {
        log::info!("Homebrew cask index unchanged (304)");
        let mut cache = cask_cache().write().await;
        cache.fetched_at = Some(std::time::Instant::now());
        return cache.index.clone();
    }

    if !status.is_success() {
        log::warn!("Homebrew cask index returned status {}", status);
        // Return cached index on error
        let cache = cask_cache().read().await;
        return cache.index.clone();
    }

    // Extract new ETag from response headers
    let new_etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    log::info!("Fetching Homebrew cask index from {} (fresh)", url);

    let json: Vec<serde_json::Value> = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse Homebrew cask index JSON: {}", e);
            let cache = cask_cache().read().await;
            return cache.index.clone();
        }
    };

    let index = build_index(&json);

    // Update cache with fresh TTL
    {
        let mut cache = cask_cache().write().await;
        cache.etag = new_etag;
        cache.index = Some(index.clone());
        cache.fetched_at = Some(std::time::Instant::now());
    }

    Some(index)
}

pub struct HomebrewApiChecker;

impl HomebrewCaskIndex {
    /// Look up an app in the index by bundle_id (primary), app path filename (fallback),
    /// or display name normalized to cask token format (third strategy).
    /// Only returns casks with real version numbers (excludes "latest").
    pub fn lookup(&self, bundle_id: &str, app_path: &Path) -> Option<&CaskVersionInfo> {
        // Primary: match by bundle ID
        if let Some(info) = self.by_bundle_id.get(&bundle_id.to_lowercase()) {
            return Some(info);
        }

        // Fallback: match by app filename
        if let Some(filename) = app_path.file_name().and_then(|f| f.to_str()) {
            let normalized = normalize_app_name(filename);
            if let Some(info) = self.by_app_name.get(&normalized) {
                return Some(info);
            }
        }

        None
    }

    /// Look up just the cask token for an app, including "latest" casks.
    /// Used for backfilling cask tokens so that `brew outdated --greedy` can detect updates.
    pub fn lookup_token(&self, bundle_id: &str, app_path: &Path) -> Option<&str> {
        // Primary: match by bundle ID
        if let Some(token) = self.all_tokens_by_bundle_id.get(&bundle_id.to_lowercase()) {
            return Some(token.as_str());
        }

        // Fallback: match by app filename
        if let Some(filename) = app_path.file_name().and_then(|f| f.to_str()) {
            let normalized = normalize_app_name(filename);
            if let Some(token) = self.all_tokens_by_app_name.get(&normalized) {
                return Some(token.as_str());
            }

            // Third strategy: normalize display name to cask token format
            // e.g. "Firefox" → "firefox", "Visual Studio Code" → "visual-studio-code"
            let token_style = display_name_to_token(&normalized);
            if let Some(token) = self.all_tokens_by_app_name.get(&token_style) {
                return Some(token.as_str());
            }
        }

        None
    }
}

/// Convert a display name to a Homebrew cask token format.
/// e.g. "Visual Studio Code" → "visual-studio-code", "Firefox" → "firefox"
fn display_name_to_token(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[async_trait]
impl UpdateChecker for HomebrewApiChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::HomebrewApi
    }

    fn can_check(&self, _bundle_id: &str, _app_path: &Path, install_source: &AppSource) -> bool {
        // Check any non-MAS app — the API covers casks broadly
        *install_source != AppSource::MacAppStore
    }

    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        _client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        // Browser extensions must not match Homebrew casks
        if is_browser_extension(bundle_id) {
            return Ok(None);
        }

        let index = match &context.homebrew_cask_index {
            Some(idx) => idx,
            None => return Ok(None),
        };

        let cask_info = match index.lookup(bundle_id, app_path) {
            Some(info) => info,
            None => return Ok(None),
        };

        let current = match current_version {
            Some(v) => v,
            None => return Ok(None),
        };

        // If this app is from a known Homebrew cask and the cask is NOT in
        // brew outdated, it's up to date — don't override with a raw version
        // comparison that can produce false positives for multi-bundle casks.
        if context.homebrew_cask_token.is_some() {
            if let Some(ref outdated_map) = context.brew_outdated {
                if !outdated_map.contains_key(&cask_info.token) {
                    return Ok(None);
                }
            }
        }

        if version_compare::is_newer(current, &cask_info.version) {
            log::debug!(
                "Homebrew API: {} has update {} -> {} (cask: {})",
                bundle_id,
                current,
                cask_info.version,
                cask_info.token
            );
            let release_notes_url = context.github_repo.as_ref()
                .map(|slug| format!("https://github.com/{}/releases", slug));
            return Ok(Some(UpdateInfo {
                bundle_id: bundle_id.to_string(),
                current_version: Some(current.to_string()),
                available_version: cask_info.version.clone(),
                source_type: UpdateSourceType::HomebrewApi,
                download_url: cask_info.url.clone(),
                release_notes_url,
                release_notes: None,
                is_paid_upgrade: false,
                notes: None,
            }));
        }

        Ok(None)
    }
}
