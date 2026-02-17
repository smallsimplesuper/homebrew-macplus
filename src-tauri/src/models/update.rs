use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSourceType {
    Sparkle,
    HomebrewCask,
    HomebrewApi,
    MacAppStore,
    GithubReleases,
    Electron,
    Keystone,
    MicrosoftAutoupdate,
    JetbrainsToolbox,
    AdobeCc,
    Mozilla,
}

impl UpdateSourceType {
    pub fn as_str(&self) -> &str {
        match self {
            UpdateSourceType::Sparkle => "sparkle",
            UpdateSourceType::HomebrewCask => "homebrew_cask",
            UpdateSourceType::HomebrewApi => "homebrew_api",
            UpdateSourceType::MacAppStore => "mas",
            UpdateSourceType::GithubReleases => "github",
            UpdateSourceType::Electron => "electron",
            UpdateSourceType::Keystone => "keystone",
            UpdateSourceType::MicrosoftAutoupdate => "microsoft_autoupdate",
            UpdateSourceType::JetbrainsToolbox => "jetbrains_toolbox",
            UpdateSourceType::AdobeCc => "adobe_cc",
            UpdateSourceType::Mozilla => "mozilla",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sparkle" => Some(UpdateSourceType::Sparkle),
            "homebrew_cask" => Some(UpdateSourceType::HomebrewCask),
            "homebrew_api" => Some(UpdateSourceType::HomebrewApi),
            "mas" => Some(UpdateSourceType::MacAppStore),
            "github" => Some(UpdateSourceType::GithubReleases),
            "electron" => Some(UpdateSourceType::Electron),
            "keystone" => Some(UpdateSourceType::Keystone),
            "microsoft_autoupdate" => Some(UpdateSourceType::MicrosoftAutoupdate),
            "jetbrains_toolbox" => Some(UpdateSourceType::JetbrainsToolbox),
            "adobe_cc" => Some(UpdateSourceType::AdobeCc),
            "mozilla" => Some(UpdateSourceType::Mozilla),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub bundle_id: String,
    pub current_version: Option<String>,
    pub available_version: String,
    pub source_type: UpdateSourceType,
    pub download_url: Option<String>,
    pub release_notes_url: Option<String>,
    pub release_notes: Option<String>,
    pub is_paid_upgrade: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub bundle_id: String,
    pub success: bool,
    pub message: Option<String>,
    pub source_type: String,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    #[serde(default)]
    pub handled_relaunch: bool,
    #[serde(default)]
    pub delegated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl UpdateStatus {
    pub fn as_str(&self) -> &str {
        match self {
            UpdateStatus::Pending => "pending",
            UpdateStatus::InProgress => "in_progress",
            UpdateStatus::Completed => "completed",
            UpdateStatus::Failed => "failed",
        }
    }
}
