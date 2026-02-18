use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppSource {
    MacAppStore,
    Homebrew,
    HomebrewFormula,
    Direct,
    Unknown,
}

impl AppSource {
    pub fn as_str(&self) -> &str {
        match self {
            AppSource::MacAppStore => "mas",
            AppSource::Homebrew => "homebrew",
            AppSource::HomebrewFormula => "homebrew_formula",
            AppSource::Direct => "direct",
            AppSource::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "mas" | "mac_app_store" => AppSource::MacAppStore,
            "homebrew" => AppSource::Homebrew,
            "homebrew_formula" => AppSource::HomebrewFormula,
            "direct" | "identified_developer" => AppSource::Direct,
            _ => AppSource::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfo {
    pub bundle_id: String,
    pub display_name: String,
    pub app_path: String,
    pub installed_version: Option<String>,
    pub bundle_version: Option<String>,
    pub icon_file: Option<String>,
    pub architectures: Option<Vec<String>>,
    pub sparkle_feed_url: Option<String>,
    pub min_system_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedApp {
    pub bundle_id: String,
    pub display_name: String,
    pub app_path: String,
    pub installed_version: Option<String>,
    pub bundle_version: Option<String>,
    pub install_source: AppSource,
    pub obtained_from: Option<String>,
    pub homebrew_cask_token: Option<String>,
    pub architectures: Option<Vec<String>>,
    pub sparkle_feed_url: Option<String>,
    pub mas_app_id: Option<String>,
    pub homebrew_formula_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSummary {
    pub id: i64,
    pub bundle_id: String,
    pub display_name: String,
    pub app_path: String,
    pub installed_version: Option<String>,
    pub install_source: String,
    pub is_ignored: bool,
    pub icon_cache_path: Option<String>,
    pub has_update: bool,
    pub available_version: Option<String>,
    pub update_source: Option<String>,
    pub homebrew_cask_token: Option<String>,
    pub sparkle_feed_url: Option<String>,
    pub obtained_from: Option<String>,
    pub homebrew_formula_name: Option<String>,
    pub release_notes: Option<String>,
    pub release_notes_url: Option<String>,
    pub update_notes: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDetail {
    pub id: i64,
    pub bundle_id: String,
    pub display_name: String,
    pub app_path: String,
    pub installed_version: Option<String>,
    pub bundle_version: Option<String>,
    pub icon_cache_path: Option<String>,
    pub architectures: Option<Vec<String>>,
    pub install_source: String,
    pub obtained_from: Option<String>,
    pub homebrew_cask_token: Option<String>,
    pub mas_app_id: Option<String>,
    pub homebrew_formula_name: Option<String>,
    pub is_ignored: bool,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub description: Option<String>,
    pub update_sources: Vec<UpdateSourceInfo>,
    pub available_update: Option<AvailableUpdateInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSourceInfo {
    pub source_type: String,
    pub source_url: Option<String>,
    pub is_primary: bool,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdateInfo {
    pub available_version: String,
    pub source_type: String,
    pub release_notes_url: Option<String>,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub is_paid_upgrade: bool,
    pub detected_at: Option<String>,
    pub notes: Option<String>,
}
