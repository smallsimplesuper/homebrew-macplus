pub mod bundle_reader;
pub mod deduplicator;
pub mod directory_scan;
pub mod homebrew;
pub mod homebrew_formula;
pub mod mas;
pub mod pkg_receipts;
pub mod spotlight;
pub mod system_profiler;

use async_trait::async_trait;

use crate::models::DetectedApp;
use crate::utils::AppResult;

#[async_trait]
pub trait AppDetector: Send + Sync {
    fn name(&self) -> &str;
    async fn detect(&self) -> AppResult<Vec<DetectedApp>>;
}

pub struct DetectionEngine {
    detectors: Vec<Box<dyn AppDetector>>,
}

impl DetectionEngine {
    pub fn new() -> Self {
        Self::with_scan_locations(Vec::new(), 2)
    }

    pub fn with_scan_locations(scan_locations: Vec<String>, scan_depth: u32) -> Self {
        Self {
            detectors: vec![
                Box::new(spotlight::SpotlightDetector),
                Box::new(directory_scan::DirectoryScanDetector::new(scan_locations, scan_depth)),
                Box::new(system_profiler::SystemProfilerDetector),
                Box::new(homebrew::HomebrewDetector),
                Box::new(homebrew_formula::HomebrewFormulaDetector),
                Box::new(mas::MasDetector),
            ],
        }
    }

    pub async fn detect_all(
        &self,
        on_progress: impl Fn(&str, usize, usize),
    ) -> AppResult<Vec<DetectedApp>> {
        let total = self.detectors.len();

        // Run all detectors concurrently
        let handles: Vec<_> = self.detectors.iter().map(|d| d.detect()).collect();
        let results = futures::future::join_all(handles).await;

        let mut all_apps = Vec::new();
        for (i, (result, detector)) in results.into_iter().zip(self.detectors.iter()).enumerate() {
            on_progress(detector.name(), i, total);
            match result {
                Ok(apps) => {
                    log::info!("{} found {} apps", detector.name(), apps.len());
                    all_apps.extend(apps);
                }
                Err(e) => {
                    log::warn!("{} failed: {}", detector.name(), e);
                }
            }
        }

        let deduped = deduplicator::deduplicate(all_apps);
        Ok(deduped)
    }
}
