use async_trait::async_trait;

use super::AppDetector;
use crate::models::{AppSource, DetectedApp};
use crate::utils::brew::brew_path;
use crate::utils::command::run_command_with_timeout;
use crate::utils::AppResult;

pub struct HomebrewFormulaDetector;

#[async_trait]
impl AppDetector for HomebrewFormulaDetector {
    fn name(&self) -> &str {
        "Homebrew Formulae"
    }

    async fn detect(&self) -> AppResult<Vec<DetectedApp>> {
        let brew = match brew_path() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let brew_str = brew.to_string_lossy().to_string();

        // Get list of installed formulae
        let list_output = run_command_with_timeout(&brew_str, &["list", "--formula"], 30).await;

        let list_output = match list_output {
            Ok(o) if o.status.success() => o,
            _ => return Ok(Vec::new()),
        };

        let formula_names: Vec<String> = String::from_utf8_lossy(&list_output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.trim().to_string())
            .collect();

        if formula_names.is_empty() {
            return Ok(Vec::new());
        }

        // Batch fetch formula info via JSON
        let mut info_args: Vec<&str> = vec!["info", "--json=v2", "--formula"];
        let name_refs: Vec<&str> = formula_names.iter().map(|s| s.as_str()).collect();
        info_args.extend(&name_refs);

        let info_output = run_command_with_timeout(&brew_str, &info_args, 30).await;

        let info_output = match info_output {
            Ok(o) if o.status.success() => o,
            _ => {
                // Fallback: return formulae without version info
                return Ok(formula_names
                    .into_iter()
                    .map(|name| make_formula_app(&name, None))
                    .collect());
            }
        };

        let json: serde_json::Value = match serde_json::from_slice(&info_output.stdout) {
            Ok(v) => v,
            Err(_) => {
                return Ok(formula_names
                    .into_iter()
                    .map(|name| make_formula_app(&name, None))
                    .collect());
            }
        };

        let formulae = json
            .get("formulae")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();

        let mut apps = Vec::new();
        for formula in &formulae {
            let name = formula
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default();

            if name.is_empty() {
                continue;
            }

            // Get installed version from the "installed" array
            let version = formula
                .get("installed")
                .and_then(|i| i.as_array())
                .and_then(|arr| arr.first())
                .and_then(|i| i.get("version"))
                .and_then(|v| v.as_str())
                .map(String::from);

            apps.push(make_formula_app(name, version.as_deref()));
        }

        Ok(apps)
    }
}

fn make_formula_app(name: &str, version: Option<&str>) -> DetectedApp {
    DetectedApp {
        bundle_id: format!("homebrew.formula.{}", name),
        display_name: name.to_string(),
        app_path: String::new(),
        installed_version: version.map(String::from),
        bundle_version: None,
        install_source: AppSource::HomebrewFormula,
        obtained_from: Some("homebrew".to_string()),
        homebrew_cask_token: None,
        architectures: None,
        sparkle_feed_url: None,
        mas_app_id: None,
        homebrew_formula_name: Some(name.to_string()),
    }
}
