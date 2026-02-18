use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    pub bundle_id: String,
    pub success: bool,
    pub message: Option<String>,
    pub running: bool,
    pub cleaned_paths: Vec<String>,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociatedFiles {
    pub paths: Vec<AssociatedFile>,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociatedFile {
    pub path: String,
    pub size_bytes: u64,
    pub kind: String,
}
