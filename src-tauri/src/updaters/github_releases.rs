use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use super::version_compare;
use super::UpdateChecker;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::http_client::APP_USER_AGENT;
use crate::utils::AppResult;

pub struct GitHubReleasesChecker;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    prerelease: bool,
    draft: bool,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[allow(dead_code)]
    content_type: Option<String>,
}

// --- ETag cache for GitHub API rate limit mitigation ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ETagCacheEntry {
    etag: String,
    response_body: String,
}

/// In-memory ETag cache keyed by "owner/repo".
fn etag_cache() -> &'static RwLock<HashMap<String, ETagCacheEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<String, ETagCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let map = load_etag_cache_from_disk().unwrap_or_default();
        RwLock::new(map)
    })
}

/// Whether we've been rate-limited this cycle (skip remaining GitHub checks).
static RATE_LIMITED: AtomicBool = AtomicBool::new(false);

/// Reset the rate-limit flag at the start of each check cycle.
pub fn reset_rate_limit_flag() {
    RATE_LIMITED.store(false, Ordering::Relaxed);
}

fn etag_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("com.macplus.app").join("github_etag_cache.json"))
}

fn load_etag_cache_from_disk() -> Option<HashMap<String, ETagCacheEntry>> {
    let path = etag_cache_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Persist the ETag cache to disk (called after each check cycle).
pub async fn save_etag_cache() {
    let cache = etag_cache().read().await;
    if cache.is_empty() {
        return;
    }
    if let Some(path) = etag_cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(&*cache) {
            let _ = std::fs::write(&path, json);
        }
    }
}

/// Built-in mapping of macOS bundle IDs to GitHub "owner/repo" slugs.
fn github_mappings() -> &'static HashMap<&'static str, &'static str> {
    static MAPPINGS: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAPPINGS.get_or_init(|| {
        let mut m = HashMap::new();
        // Terminal emulators
        m.insert("com.googlecode.iterm2", "gnachman/iTerm2");
        m.insert("com.mitchellh.ghostty", "ghostty-org/ghostty");
        m.insert("io.alacritty", "alacritty/alacritty");
        m.insert("co.zeit.hyper", "vercel/hyper");
        m.insert("com.github.wez.wezterm", "wez/wezterm");

        // Window management
        m.insert("com.knollsoft.Rectangle", "rxhanson/Rectangle");
        m.insert("com.lwouis.alt-tab-macos", "lwouis/alt-tab-macos");
        m.insert("com.amethyst.Amethyst", "ianyh/Amethyst");
        m.insert("org.pqrs.Karabiner-Elements", "pqrs-org/Karabiner-Elements");

        // Dev tools
        m.insert("com.vscodium", "VSCodium/vscodium");
        m.insert("abnerworks.Typora", "typora/typora");
        m.insert("com.sublimemerge", "nickedname/sublime-merge");

        // Browsers
        m.insert("org.chromium.Chromium", "nicedoc/chromium");

        // Productivity
        m.insert("com.toggl.danern.TogglDesktop", "toggl-open-source/toggldesktop");
        m.insert("org.keepassxc.keepassxc", "keepassxreboot/keepassxc");
        m.insert("com.bitwarden.desktop", "bitwarden/clients");
        m.insert("md.obsidian", "obsidianmd/obsidian-releases");
        m.insert("com.logseq.logseq", "logseq/logseq");
        m.insert("org.zettlr.app", "Zettlr/Zettlr");
        m.insert("org.joplinapp.desktop", "laurent22/joplin");
        m.insert("com.standardnotes.app", "standardnotes/app");

        // Media
        m.insert("org.videolan.vlc", "videolan/vlc");
        m.insert("com.colliderli.iina", "iina/iina");
        m.insert("com.obsproject.obs-studio", "obsproject/obs-studio");
        m.insert("org.audacityteam.audacity", "audacity/audacity");
        m.insert("org.gimp.gimp-official", "GNOME/gimp");
        m.insert("org.inkscape.Inkscape", "inkscape/inkscape");

        // Database
        m.insert("io.dbeaver.DBeaverCommunity", "dbeaver/dbeaver");

        // Chat / Communication
        m.insert("com.signalos.Signal", "nicedoc/Signal-Desktop");
        m.insert("com.hnc.Discord", "nicedoc/discord");

        // System utilities
        m.insert("com.objective-see.lulu.app", "objective-see/LuLu");
        m.insert("com.gaosun.eul", "gao-sun/eul");
        m.insert("com.p0deje.Maccy", "p0deje/Maccy");
        m.insert("com.jordanbaird.Ice", "jordanbaird/Ice");
        m.insert("org.pqrs.Tinkertool", "pqrs-org/Karabiner-Elements");
        m.insert("com.linearmouse.LinearMouse", "linearmouse/linearmouse");
        m.insert("com.MonitorControl.MonitorControl", "MonitorControl/MonitorControl");
        m.insert("com.dwarvesv.minimalbar", "nicedoc/hidden");

        // File management
        m.insert("com.keka.Keka", "aonez/Keka");

        // Dev tools (additional)
        m.insert("dev.zed.Zed", "zed-industries/zed");
        m.insert("com.insomnia.app", "Kong/insomnia");
        m.insert("com.postmanlabs.mac", "postmanlabs/postman-app-support");
        m.insert("com.todesktop.230313mzl4w4u92", "getcursor/cursor");

        // System utilities (additional)
        m.insert("com.exelban.stats", "exelban/stats");
        m.insert("org.hammerspoon.Hammerspoon", "Hammerspoon/hammerspoon");
        m.insert("com.knollsoft.Hookshot", "rxhanson/Rectangle");

        // Media (additional)
        m.insert("fr.handbrake.HandBrake", "HandBrake/HandBrake");
        m.insert("net.kovidgoyal.calibre", "kovidgoyal/calibre");
        m.insert("com.ImageOptim.ImageOptim", "ImageOptim/ImageOptim");

        // Communication (additional)
        m.insert("im.riot.app", "element-hq/element-desktop");
        m.insert("org.mattermost.desktop", "mattermost/desktop");

        // Security / VPN
        m.insert("org.cryptomator", "cryptomator/cryptomator");
        m.insert("net.tunnelblick.tunnelblick", "Tunnelblick/Tunnelblick");
        m.insert("net.mullvad.vpn", "mullvad/mullvadvpn-app");
        m.insert("com.wireguard.macos", "WireGuard/wireguard-apple");

        // Dev tools (more)
        m.insert("com.github.GitHubClient", "desktop/desktop");
        m.insert("com.lapce", "lapce/lapce");
        m.insert("io.httpie.desktop", "httpie/desktop");
        m.insert("com.hoppscotch.desktop", "hoppscotch/hoppscotch");
        m.insert("com.neovide.neovide", "neovide/neovide");
        m.insert("com.helix-editor.Helix", "helix-editor/helix");

        // Productivity (more)
        m.insert("net.ankiweb.dtop", "ankitects/anki");
        m.insert("com.anytype.anytype", "anyproto/anytype-ts");
        m.insert("com.appflowy.appflowy", "AppFlowy-IO/AppFlowy");

        // System utilities (more)
        m.insert("info.eurocomp.MeetingBar", "leits/MeetingBar");
        m.insert("org.pqrs.Karabiner-EventViewer", "pqrs-org/Karabiner-Elements");
        m.insert("com.alienator88.Pearcleaner", "alienator88/Pearcleaner");
        m.insert("org.th-ch.YTMusic", "th-ch/youtube-music");
        m.insert("com.ther0n.UnnaturalScrollWheels", "ther0n/UnnaturalScrollWheels");
        m.insert("com.jamiepinheiro.caffeine", "IntelliScape/caffeine");

        // Media (more)
        m.insert("app.museeks.museeks", "martpie/museeks");
        m.insert("com.jellyfin.macos", "jellyfin/jellyfin-media-player");
        m.insert("org.shotcut.Shotcut", "mltframework/shotcut");
        m.insert("com.kdenlive", "KDE/kdenlive");

        // File management (more)
        m.insert("ch.sudo.cyberduck", "iterate-ch/cyberduck");

        // Communication (more)
        m.insert("com.jitsi.osx", "jitsi/jitsi-meet-electron");
        m.insert("com.zulipchat.zulip-electron", "zulip/zulip-desktop");

        // Science / Engineering
        m.insert("org.freecadweb.FreeCAD", "FreeCAD/FreeCAD");
        m.insert("org.openscad.OpenSCAD", "openscad/openscad");

        // Gaming / Emulation
        m.insert("org.ppsspp.PPSSPP", "hrydgard/ppsspp");
        m.insert("org.DolphinEmu.dolphin-emu", "dolphin-emu/dolphin");
        m.insert("com.citra-emu.citra", "citra-emu/citra");

        // Database clients
        m.insert("io.beekeeperstudio.desktop", "beekeeper-studio/beekeeper-studio");

        // Document editing
        m.insert("org.texstudio.texstudio", "texstudio-org/texstudio");

        m
    })
}

/// Find the best macOS-compatible asset from a GitHub release.
fn find_macos_asset(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
    let macos_keywords = ["macos", "mac", "darwin", "osx", "universal", "arm64", "aarch64", "x86_64"];
    let good_extensions = [".dmg", ".zip", ".pkg"];

    // First pass: look for assets with macOS keywords and good extensions
    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        let has_mac_keyword = macos_keywords.iter().any(|kw| name_lower.contains(kw));
        let has_good_ext = good_extensions.iter().any(|ext| name_lower.ends_with(ext));

        if has_mac_keyword && has_good_ext {
            // Prefer universal/arm64 builds
            if name_lower.contains("universal") || name_lower.contains("arm64") || name_lower.contains("aarch64") {
                return Some(asset);
            }
        }
    }

    // Second pass: any asset with macOS keyword and good extension
    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        let has_mac_keyword = macos_keywords.iter().any(|kw| name_lower.contains(kw));
        let has_good_ext = good_extensions.iter().any(|ext| name_lower.ends_with(ext));

        if has_mac_keyword && has_good_ext {
            return Some(asset);
        }
    }

    // Third pass: DMG/PKG without platform keywords (many mac-only apps don't specify)
    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        if name_lower.ends_with(".dmg") || name_lower.ends_with(".pkg") {
            // Exclude obvious non-mac assets
            let is_non_mac = name_lower.contains("linux") || name_lower.contains("windows") || name_lower.contains(".exe") || name_lower.contains(".deb") || name_lower.contains(".rpm");
            if !is_non_mac {
                return Some(asset);
            }
        }
    }

    None
}

#[async_trait]
impl UpdateChecker for GitHubReleasesChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::GithubReleases
    }

    fn can_check(&self, _bundle_id: &str, _app_path: &Path, _install_source: &AppSource) -> bool {
        // Always return true; check() resolves the repo from context or hardcoded map
        // and returns Ok(None) immediately if no mapping exists.
        true
    }

    async fn check(
        &self,
        bundle_id: &str,
        _app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        // Resolve repo: user override (from context) > built-in map
        let repo_slug = if let Some(ref repo) = context.github_repo {
            repo.clone()
        } else if let Some(slug) = github_mappings().get(bundle_id) {
            slug.to_string()
        } else {
            return Ok(None);
        };

        let parts: Vec<&str> = repo_slug.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Ok(None);
        }

        check_github_release(parts[0], parts[1], bundle_id, current_version, client).await
    }
}

pub async fn check_github_release(
    owner: &str,
    repo: &str,
    bundle_id: &str,
    current_version: Option<&str>,
    client: &reqwest::Client,
) -> AppResult<Option<UpdateInfo>> {
    // Skip if we've been rate-limited this cycle
    if RATE_LIMITED.load(Ordering::Relaxed) {
        return Ok(None);
    }

    let cache_key = format!("{}/{}", owner, repo);
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );

    // Check for cached ETag
    let cached_etag = {
        let cache = etag_cache().read().await;
        cache.get(&cache_key).map(|e| e.etag.clone())
    };

    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", APP_USER_AGENT);

    if let Some(ref etag) = cached_etag {
        req = req.header("If-None-Match", etag.as_str());
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            log::debug!("GitHub API request failed for {}: {}", cache_key, e);
            return Ok(None);
        }
    };

    let status = resp.status();

    // Handle rate limiting (403 with X-RateLimit-Remaining: 0)
    if status == reqwest::StatusCode::FORBIDDEN {
        let remaining = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok());

        if remaining == Some(0) {
            log::warn!("GitHub API rate limit reached, skipping remaining GitHub checks");
            RATE_LIMITED.store(true, Ordering::Relaxed);
        }
        return Ok(None);
    }

    // 304 Not Modified -- use cached response (doesn't count against rate limit)
    if status == reqwest::StatusCode::NOT_MODIFIED {
        let cache = etag_cache().read().await;
        if let Some(entry) = cache.get(&cache_key) {
            if let Ok(release) = serde_json::from_str::<GitHubRelease>(&entry.response_body) {
                return parse_github_release(release, bundle_id, current_version, owner, repo);
            }
        }
        return Ok(None);
    }

    if !status.is_success() {
        return Ok(None);
    }

    // Extract new ETag
    let new_etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let body = resp.text().await?;

    // Cache the response with ETag
    if let Some(etag) = new_etag {
        let mut cache = etag_cache().write().await;
        cache.insert(
            cache_key,
            ETagCacheEntry {
                etag,
                response_body: body.clone(),
            },
        );
    }

    let release: GitHubRelease = serde_json::from_str(&body)
        .map_err(|e| crate::utils::AppError::Custom(format!("GitHub JSON parse error: {}", e)))?;
    parse_github_release(release, bundle_id, current_version, owner, repo)
}

fn parse_github_release(
    release: GitHubRelease,
    bundle_id: &str,
    current_version: Option<&str>,
    owner: &str,
    repo: &str,
) -> AppResult<Option<UpdateInfo>> {
    if release.draft || release.prerelease {
        return Ok(None);
    }

    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);

    if let Some(current) = current_version {
        if version_compare::is_newer(current, version) {
            let download_url = find_macos_asset(&release.assets)
                .map(|a| a.browser_download_url.clone());

            log::info!(
                "GitHub: {} has update {} -> {} ({}/{})",
                bundle_id,
                current,
                version,
                owner,
                repo
            );

            return Ok(Some(UpdateInfo {
                bundle_id: bundle_id.to_string(),
                current_version: Some(current.to_string()),
                available_version: version.to_string(),
                source_type: UpdateSourceType::GithubReleases,
                download_url,
                release_notes_url: Some(release.html_url),
                release_notes: release.body,
                is_paid_upgrade: false,
                notes: None,
            }));
        }
    }

    Ok(None)
}
