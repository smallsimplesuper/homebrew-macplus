use std::path::Path;

use crate::models::{AppSource, BundleInfo};
use crate::utils::plist_parser::{get_string, read_info_plist};

pub fn read_bundle(app_path: &Path) -> Option<BundleInfo> {
    let dict = read_info_plist(app_path).ok()?;

    let bundle_id = get_string(&dict, "CFBundleIdentifier")?;
    let display_name = get_string(&dict, "CFBundleDisplayName")
        .or_else(|| get_string(&dict, "CFBundleName"))
        .unwrap_or_else(|| {
            app_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    Some(BundleInfo {
        bundle_id,
        display_name,
        app_path: app_path.to_string_lossy().to_string(),
        installed_version: get_string(&dict, "CFBundleShortVersionString"),
        bundle_version: get_string(&dict, "CFBundleVersion"),
        icon_file: get_string(&dict, "CFBundleIconFile")
            .or_else(|| get_string(&dict, "CFBundleIconName")),
        architectures: None,
        sparkle_feed_url: get_string(&dict, "SUFeedURL"),
        min_system_version: get_string(&dict, "LSMinimumSystemVersion"),
    })
}

pub fn has_sparkle_framework(app_path: &Path) -> bool {
    app_path
        .join("Contents/Frameworks/Sparkle.framework")
        .exists()
}

pub fn has_mas_receipt(app_path: &Path) -> bool {
    app_path.join("Contents/_MASReceipt/receipt").exists()
}

pub fn is_electron_app(app_path: &Path) -> bool {
    app_path
        .join("Contents/Frameworks/Electron Framework.framework")
        .exists()
}

pub fn detect_install_source(app_path: &Path) -> AppSource {
    if has_mas_receipt(app_path) {
        AppSource::MacAppStore
    } else {
        AppSource::Direct
    }
}
