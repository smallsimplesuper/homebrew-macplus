use async_trait::async_trait;
use std::process::Command;

use super::AppDetector;
use crate::models::DetectedApp;
use crate::utils::AppResult;

pub struct PkgReceiptsDetector;

#[async_trait]
impl AppDetector for PkgReceiptsDetector {
    fn name(&self) -> &str {
        "Package Receipts"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        // pkg receipts are used as supplementary data rather than primary detection
        // since they don't directly map to .app bundles easily.
        // For now, we rely on other detectors and use pkg receipts
        // for additional metadata in future phases.
        Ok(Vec::new())
    }
}

pub fn get_pkg_version(package_id: &str) -> Option<String> {
    let output = Command::new("pkgutil")
        .args(["--pkg-info", package_id])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().find_map(|line| {
        if let Some(version) = line.strip_prefix("version: ") {
            Some(version.trim().to_string())
        } else {
            None
        }
    })
}
