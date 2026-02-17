use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::utils::http_client::APP_USER_AGENT;

/// Result of a SHA-256 change detection check for a "latest" cask.
#[derive(Debug, Clone)]
pub enum CaskShaResult {
    /// The SHA changed since the last check — update likely available.
    Changed,
    /// The SHA is the same — no update detected.
    Unchanged,
    /// The cask uses `:no_check` — cannot detect updates via SHA.
    NoCheck,
    /// First time checking this cask — stored initial SHA, no comparison possible.
    FirstSeen,
    /// Failed to fetch or parse the cask file.
    Error(String),
}

/// Check if a "latest" cask has been updated by comparing the SHA-256 hash
/// in its Ruby cask file on GitHub against our cached value.
///
/// This is the technique MacUpdater uses: for casks with `version "latest"`,
/// the only signal of an update is a change in the `sha256` line.
pub async fn check_cask_sha(
    cask_token: &str,
    client: &reqwest::Client,
    db: &Arc<Mutex<Database>>,
) -> CaskShaResult {
    let first_letter = match cask_token.chars().next() {
        Some(c) => c,
        None => return CaskShaResult::Error("empty cask token".to_string()),
    };

    let url = format!(
        "https://raw.githubusercontent.com/Homebrew/homebrew-cask/master/Casks/{}/{}.rb",
        first_letter, cask_token
    );

    let resp = match client
        .get(&url)
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return CaskShaResult::Error(format!("fetch failed: {}", e)),
    };

    if !resp.status().is_success() {
        return CaskShaResult::Error(format!("HTTP {}", resp.status()));
    }

    let body = match resp.text().await {
        Ok(t) => t,
        Err(e) => return CaskShaResult::Error(format!("read body failed: {}", e)),
    };

    // Parse the sha256 line from the Ruby cask file
    let current_sha = extract_sha256(&body);

    match current_sha {
        None => {
            // Check if it's :no_check
            if body.contains(":no_check") {
                log::info!(
                    "Cask SHA check for {}: sha256 :no_check — cannot detect updates",
                    cask_token
                );
                CaskShaResult::NoCheck
            } else {
                CaskShaResult::Error("no sha256 line found in cask file".to_string())
            }
        }
        Some(sha) => {
            let db_guard = db.lock().await;
            let cached = db_guard.get_cask_sha(cask_token);

            match cached {
                None => {
                    // First time seeing this cask — store the SHA
                    let _ = db_guard.set_cask_sha(cask_token, &sha);
                    log::info!(
                        "Cask SHA check for {}: first seen, stored SHA {}...{}",
                        cask_token,
                        &sha[..8],
                        &sha[56..]
                    );
                    CaskShaResult::FirstSeen
                }
                Some(old_sha) => {
                    if old_sha == sha {
                        log::info!(
                            "Cask SHA check for {}: unchanged ({}...)",
                            cask_token,
                            &sha[..8]
                        );
                        CaskShaResult::Unchanged
                    } else {
                        // SHA changed — update the cache and report
                        let _ = db_guard.set_cask_sha(cask_token, &sha);
                        log::info!(
                            "Cask SHA check for {}: sha256 changed → update available ({}... → {}...)",
                            cask_token,
                            &old_sha[..8.min(old_sha.len())],
                            &sha[..8]
                        );
                        CaskShaResult::Changed
                    }
                }
            }
        }
    }
}

/// Extract a hex SHA-256 hash from a Ruby cask file.
/// Matches lines like: `sha256 "abc123..."`
fn extract_sha256(ruby_content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"sha256\s+"([a-f0-9]{64})""#).ok()?;
    re.captures(ruby_content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}
