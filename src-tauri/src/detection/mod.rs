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
use futures::stream::{FuturesUnordered, StreamExt};

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

        // Run all detectors concurrently with FuturesUnordered for real-time progress
        let mut futures: FuturesUnordered<_> = self
            .detectors
            .iter()
            .map(|d| {
                let name = d.name().to_string();
                async move { (name, d.detect().await) }
            })
            .collect();

        let mut all_apps = Vec::new();
        let mut completed = 0usize;

        while let Some((name, result)) = futures.next().await {
            completed += 1;
            on_progress(&name, completed, total);
            match result {
                Ok(apps) => {
                    log::info!("{} found {} apps", name, apps.len());
                    all_apps.extend(apps);
                }
                Err(e) => {
                    log::warn!("{} failed: {}", name, e);
                }
            }
        }

        let deduped = deduplicator::deduplicate(all_apps);
        Ok(deduped)
    }
}
