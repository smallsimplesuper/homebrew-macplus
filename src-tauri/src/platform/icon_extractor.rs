use std::path::Path;
use std::process::{Command, Stdio};

use crate::utils::{plist_parser, AppResult};

/// Extract app icon as PNG bytes using a multi-strategy fallback chain.
///
/// 1. `sips` with `CFBundleIconFile` (traditional `.icns` files)
/// 2. Glob for any `.icns` in `Contents/Resources/`
/// 3. `qlmanage` thumbnail (universal fallback — works with Asset Catalogs, etc.)
pub fn extract_icon_png(app_path: &Path, output_dir: &Path) -> AppResult<Option<String>> {
    // Determine bundle_id for the output filename
    let bundle_id = plist_parser::read_info_plist(app_path)
        .ok()
        .and_then(|dict| plist_parser::get_string(&dict, "CFBundleIdentifier"))
        .unwrap_or_else(|| "unknown".to_string());

    let output_path = output_dir.join(format!("{}.png", bundle_id));

    // Early return if icon PNG already exists in cache
    if output_path.exists() {
        log::debug!("Icon already cached for {}", bundle_id);
        return Ok(Some(output_path.to_string_lossy().to_string()));
    }

    // Strategy 1: CFBundleIconFile via sips
    if let Some(path) = try_sips_cfbundle_icon_file(app_path, &output_path, &bundle_id) {
        return Ok(Some(path));
    }

    // Strategy 2: Glob for any .icns in Resources
    if let Some(path) = try_glob_icns(app_path, &output_path, &bundle_id) {
        return Ok(Some(path));
    }

    // Strategy 3: qlmanage thumbnail (universal fallback)
    if let Some(path) = try_qlmanage(app_path, &output_path, &bundle_id) {
        return Ok(Some(path));
    }

    log::warn!("All icon extraction strategies failed for {} ({})", bundle_id, app_path.display());
    Ok(None)
}

/// Strategy 1: Use CFBundleIconFile (NOT CFBundleIconName) to find a .icns file
/// in Contents/Resources/, then convert with sips.
fn try_sips_cfbundle_icon_file(app_path: &Path, output_path: &Path, bundle_id: &str) -> Option<String> {
    let dict = plist_parser::read_info_plist(app_path).ok()?;
    // Only use CFBundleIconFile — CFBundleIconName refers to asset catalog entries
    let icon_name = plist_parser::get_string(&dict, "CFBundleIconFile")?;

    let mut icon_path = app_path.join("Contents/Resources").join(&icon_name);
    if icon_path.extension().is_none() {
        icon_path.set_extension("icns");
    }

    if !icon_path.exists() {
        log::debug!("[{}] Strategy 1: CFBundleIconFile '{}' not found at {}", bundle_id, icon_name, icon_path.display());
        return None;
    }

    convert_icns_with_sips(&icon_path, output_path, bundle_id, 1)
}

/// Strategy 2: Glob for any .icns file in Contents/Resources/.
/// Prefer AppIcon.icns if present, otherwise use the first match.
fn try_glob_icns(app_path: &Path, output_path: &Path, bundle_id: &str) -> Option<String> {
    let resources_dir = app_path.join("Contents/Resources");
    if !resources_dir.is_dir() {
        log::debug!("[{}] Strategy 2: No Contents/Resources directory", bundle_id);
        return None;
    }

    let entries: Vec<_> = std::fs::read_dir(&resources_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("icns"))
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        log::debug!("[{}] Strategy 2: No .icns files found in Resources", bundle_id);
        return None;
    }

    // Prefer AppIcon.icns if present
    let icns_path = entries
        .iter()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("AppIcon.icns")
        })
        .unwrap_or(&entries[0])
        .path();

    convert_icns_with_sips(&icns_path, output_path, bundle_id, 2)
}

/// Strategy 3: Use qlmanage to generate a Quick Look thumbnail.
/// Works for ALL apps regardless of icon storage format (asset catalogs, tiff, icns, etc.)
fn try_qlmanage(app_path: &Path, output_path: &Path, bundle_id: &str) -> Option<String> {
    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            log::debug!("[{}] Strategy 3: Failed to create temp dir: {}", bundle_id, e);
            return None;
        }
    };

    let status = Command::new("qlmanage")
        .args([
            "-t",
            "-s",
            "128",
            "-o",
            &tmp_dir.path().to_string_lossy(),
            &app_path.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match status {
        Ok(output) if output.status.success() => {
            // qlmanage outputs a file named <input_name>.png in the output dir
            let entries: Vec<_> = std::fs::read_dir(tmp_dir.path())
                .ok()?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "png")
                        .unwrap_or(false)
                })
                .collect();

            if let Some(png_entry) = entries.first() {
                if std::fs::copy(png_entry.path(), output_path).is_ok() {
                    log::debug!("[{}] Strategy 3 (qlmanage): success", bundle_id);
                    return Some(output_path.to_string_lossy().to_string());
                }
            }

            log::debug!("[{}] Strategy 3: qlmanage succeeded but no PNG found in output", bundle_id);
            None
        }
        Ok(_) => {
            log::debug!("[{}] Strategy 3: qlmanage exited with non-zero status", bundle_id);
            None
        }
        Err(e) => {
            log::debug!("[{}] Strategy 3: qlmanage failed to execute: {}", bundle_id, e);
            None
        }
    }
}

/// Helper: convert a .icns file to 128x128 PNG using sips.
fn convert_icns_with_sips(icns_path: &Path, output_path: &Path, bundle_id: &str, strategy: u8) -> Option<String> {
    let status = Command::new("sips")
        .args([
            "-s",
            "format",
            "png",
            "-z",
            "128",
            "128",
            &icns_path.to_string_lossy(),
            "--out",
            &output_path.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match status {
        Ok(output) if output.status.success() => {
            log::debug!("[{}] Strategy {} (sips): success from {}", bundle_id, strategy, icns_path.display());
            Some(output_path.to_string_lossy().to_string())
        }
        Ok(_) => {
            log::debug!("[{}] Strategy {}: sips failed for {}", bundle_id, strategy, icns_path.display());
            None
        }
        Err(e) => {
            log::debug!("[{}] Strategy {}: sips command error: {}", bundle_id, strategy, e);
            None
        }
    }
}
