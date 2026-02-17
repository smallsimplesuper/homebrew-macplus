use async_trait::async_trait;
use std::path::Path;

use super::version_compare;
use super::UpdateChecker;
use crate::detection::bundle_reader;
use crate::models::{AppSource, UpdateInfo, UpdateSourceType};
use crate::utils::{plist_parser, AppResult};

pub struct SparkleChecker;

#[async_trait]
impl UpdateChecker for SparkleChecker {
    fn source_type(&self) -> UpdateSourceType {
        UpdateSourceType::Sparkle
    }

    fn can_check(&self, _bundle_id: &str, app_path: &Path, install_source: &AppSource) -> bool {
        if *install_source == AppSource::MacAppStore {
            return false;
        }
        // Check for Sparkle framework or SUFeedURL
        bundle_reader::has_sparkle_framework(app_path)
            || plist_parser::read_info_plist(app_path)
                .ok()
                .and_then(|d| plist_parser::get_string(&d, "SUFeedURL"))
                .is_some()
    }

    async fn check(
        &self,
        bundle_id: &str,
        app_path: &Path,
        current_version: Option<&str>,
        client: &reqwest::Client,
        context: &super::AppCheckContext,
    ) -> AppResult<Option<UpdateInfo>> {
        // Prefer feed URL from context (DB), fall back to plist
        let feed_url = if let Some(ref url) = context.sparkle_feed_url {
            url.clone()
        } else {
            let dict = plist_parser::read_info_plist(app_path)?;
            plist_parser::get_string(&dict, "SUFeedURL")
                .ok_or_else(|| crate::utils::AppError::NotFound("No SUFeedURL found".into()))?
        };

        let response = client.get(&feed_url).send().await?;
        let body = response.text().await?;

        let update = parse_appcast(&body, bundle_id, current_version)?;
        Ok(update)
    }
}

/// Pre-release indicator strings (case-insensitive check)
const PRE_RELEASE_INDICATORS: &[&str] = &[
    "beta", "alpha", "rc", "dev", "pre", "nightly", "canary",
];

/// Returns true if the version string or title looks like a pre-release.
fn is_pre_release(version: &str, title: Option<&str>) -> bool {
    let version_lower = version.to_lowercase();
    let is_beta = PRE_RELEASE_INDICATORS
        .iter()
        .any(|ind| version_lower.contains(ind));
    if is_beta {
        return true;
    }
    if let Some(t) = title {
        let title_lower = t.to_lowercase();
        PRE_RELEASE_INDICATORS
            .iter()
            .any(|ind| title_lower.contains(ind))
    } else {
        false
    }
}

fn parse_appcast(
    xml: &str,
    bundle_id: &str,
    current_version: Option<&str>,
) -> AppResult<Option<UpdateInfo>> {
    // Primary: parse raw XML for Sparkle <enclosure> tags (correct download URLs)
    let best_version = parse_sparkle_enclosures(xml, current_version);

    // Fallback: use feed-rs if enclosure parsing found nothing
    let best_version = if best_version.is_some() {
        best_version
    } else {
        parse_with_feed_rs(xml, current_version)?
    };

    Ok(best_version.map(|(version, download_url, release_notes_url)| UpdateInfo {
        bundle_id: bundle_id.to_string(),
        current_version: current_version.map(String::from),
        available_version: version,
        source_type: UpdateSourceType::Sparkle,
        download_url,
        release_notes_url,
        release_notes: None,
        is_paid_upgrade: false,
        notes: None,
    }))
}

/// Fallback parser using feed-rs for RSS/Atom feeds.
fn parse_with_feed_rs(
    xml: &str,
    current_version: Option<&str>,
) -> AppResult<Option<(String, Option<String>, Option<String>)>> {
    let feed = feed_rs::parser::parse(xml.as_bytes())
        .map_err(|e| crate::utils::AppError::Xml(e.to_string()))?;

    let mut best_version: Option<(String, Option<String>, Option<String>)> = None;

    for entry in &feed.entries {
        let title = entry.title.as_ref().map(|t| t.content.as_str());

        for link in &entry.links {
            let href = &link.href;

            let version = entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_default();

            let ver = extract_version_from_title(&version).unwrap_or(version);

            if ver.is_empty() || is_pre_release(&ver, title) {
                continue;
            }

            if let Some(current) = current_version {
                if version_compare::is_newer(current, &ver) {
                    match &best_version {
                        Some((existing_ver, _, _)) => {
                            if version_compare::is_newer(existing_ver, &ver) {
                                best_version = Some((
                                    ver,
                                    Some(href.clone()),
                                    entry.links.first().map(|l| l.href.clone()),
                                ));
                            }
                        }
                        None => {
                            best_version = Some((
                                ver,
                                Some(href.clone()),
                                entry.links.first().map(|l| l.href.clone()),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(best_version)
}

/// Primary parser: extracts version and download URL from Sparkle <enclosure> tags.
/// Handles both single-line and multiline <enclosure .../> elements.
fn parse_sparkle_enclosures(
    xml: &str,
    current_version: Option<&str>,
) -> Option<(String, Option<String>, Option<String>)> {
    let mut best: Option<(String, Option<String>, Option<String>)> = None;

    // Collect enclosure element blocks (may span multiple lines)
    let enclosure_blocks = collect_enclosure_blocks(xml);

    // Also extract releaseNotesLink from <item> blocks
    let item_notes_links = collect_release_notes_links(xml);

    for (idx, block) in enclosure_blocks.iter().enumerate() {
        // Try sparkle:shortVersionString first, fall back to sparkle:version
        let short_ver = extract_attr(block, "sparkle:shortVersionString")
            .or_else(|| extract_attr(block, "sparkle:version"));
        let url = extract_attr(block, "url");

        let ver = match short_ver {
            Some(v) => v,
            None => continue,
        };

        // Filter pre-release versions
        if is_pre_release(&ver, None) {
            continue;
        }

        // Try releaseNotesLink from enclosure attribute first, then from item-level element
        let notes_url = extract_attr(block, "sparkle:releaseNotesLink")
            .or_else(|| item_notes_links.get(idx).cloned().flatten());

        if let Some(current) = current_version {
            if version_compare::is_newer(current, &ver) {
                match &best {
                    Some((existing, _, _)) => {
                        if version_compare::is_newer(existing, &ver) {
                            best = Some((ver, url, notes_url));
                        }
                    }
                    None => {
                        best = Some((ver, url, notes_url));
                    }
                }
            }
        } else {
            // No current version to compare, take the first one
            if best.is_none() {
                best = Some((ver, url, notes_url));
            }
        }
    }

    best
}

/// Collects <enclosure ...> blocks from raw XML, handling both single-line
/// and multiline elements (terminated by `/>` or `>`).
fn collect_enclosure_blocks(xml: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current_block: Option<String> = None;

    for line in xml.lines() {
        if let Some(ref mut block) = current_block {
            // We're inside a multiline enclosure element
            block.push(' ');
            block.push_str(line.trim());
            if line.contains("/>") || line.contains('>') {
                blocks.push(block.clone());
                current_block = None;
            }
        } else if let Some(start_idx) = line.find("<enclosure") {
            let rest = &line[start_idx..];
            if rest.contains("/>") || rest.matches('>').count() > 0 && rest.contains("url") {
                // Single-line enclosure
                blocks.push(rest.to_string());
            } else {
                // Multiline enclosure — start collecting
                current_block = Some(rest.to_string());
            }
        }
    }

    blocks
}

/// Collects `<sparkle:releaseNotesLink>` URLs from each `<item>` block,
/// indexed to match enclosure order.
fn collect_release_notes_links(xml: &str) -> Vec<Option<String>> {
    let mut links = Vec::new();
    let mut in_item = false;
    let mut current_link: Option<String> = None;

    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<item") {
            in_item = true;
            current_link = None;
        } else if trimmed == "</item>" {
            if in_item {
                links.push(current_link.take());
            }
            in_item = false;
        } else if in_item && current_link.is_none() {
            // Look for <sparkle:releaseNotesLink> element
            if let Some(start) = trimmed.find("<sparkle:releaseNotesLink>") {
                let after = &trimmed[start + "<sparkle:releaseNotesLink>".len()..];
                if let Some(end) = after.find("</sparkle:releaseNotesLink>") {
                    let url = after[..end].trim().to_string();
                    if !url.is_empty() {
                        current_link = Some(url);
                    }
                }
            }
        }
    }

    links
}

fn extract_attr(text: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = text.find(&pattern)?;
    let after = &text[start + pattern.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn extract_version_from_title(title: &str) -> Option<String> {
    // Look for version-like patterns: "Version 1.2.3" or "v1.2.3" or just "1.2.3"
    let stripped = title
        .trim()
        .strip_prefix("Version ")
        .or_else(|| title.trim().strip_prefix("v"))
        .unwrap_or(title.trim());

    // Check if it looks like a version number
    if stripped.chars().next()?.is_ascii_digit() {
        Some(stripped.to_string())
    } else {
        None
    }
}
