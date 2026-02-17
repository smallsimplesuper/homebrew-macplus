use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub phase: String,
    pub current: usize,
    pub total: usize,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanComplete {
    pub app_count: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckProgress {
    pub checked: usize,
    pub total: usize,
    pub current_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFound {
    pub bundle_id: String,
    pub current_version: Option<String>,
    pub available_version: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckComplete {
    pub updates_found: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExecuteProgress {
    pub bundle_id: String,
    pub phase: String,
    pub percent: u8,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExecuteComplete {
    pub bundle_id: String,
    pub display_name: String,
    pub success: bool,
    pub message: Option<String>,
    pub needs_relaunch: bool,
    pub app_path: Option<String>,
    #[serde(default)]
    pub delegated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHistoryEntry {
    pub id: i64,
    pub bundle_id: String,
    pub display_name: String,
    pub icon_cache_path: Option<String>,
    pub from_version: String,
    pub to_version: String,
    pub source_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
