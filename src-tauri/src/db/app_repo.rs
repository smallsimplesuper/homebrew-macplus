use std::collections::HashMap;

use crate::db::Database;
use crate::models::{AppDetail, AppSummary, AvailableUpdateInfo, DetectedApp, UpdateSourceInfo};
use crate::utils::AppResult;

impl Database {
    pub fn upsert_app(&self, app: &DetectedApp) -> AppResult<i64> {
        self.conn.execute(
            "INSERT INTO apps (bundle_id, display_name, app_path, installed_version, bundle_version, install_source, obtained_from, homebrew_cask_token, architectures, sparkle_feed_url, mas_app_id, homebrew_formula_name, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
             ON CONFLICT(bundle_id) DO UPDATE SET
                display_name = excluded.display_name,
                app_path = excluded.app_path,
                installed_version = COALESCE(excluded.installed_version, apps.installed_version),
                bundle_version = COALESCE(excluded.bundle_version, apps.bundle_version),
                install_source = CASE WHEN excluded.install_source != 'unknown' THEN excluded.install_source ELSE apps.install_source END,
                obtained_from = COALESCE(excluded.obtained_from, apps.obtained_from),
                homebrew_cask_token = COALESCE(excluded.homebrew_cask_token, apps.homebrew_cask_token),
                architectures = COALESCE(excluded.architectures, apps.architectures),
                sparkle_feed_url = COALESCE(excluded.sparkle_feed_url, apps.sparkle_feed_url),
                mas_app_id = COALESCE(excluded.mas_app_id, apps.mas_app_id),
                homebrew_formula_name = COALESCE(excluded.homebrew_formula_name, apps.homebrew_formula_name),
                last_seen_at = datetime('now')",
            rusqlite::params![
                app.bundle_id,
                app.display_name,
                app.app_path,
                app.installed_version,
                app.bundle_version,
                app.install_source.as_str(),
                app.obtained_from,
                app.homebrew_cask_token,
                app.architectures.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
                app.sparkle_feed_url,
                app.mas_app_id,
                app.homebrew_formula_name,
            ],
        )?;

        let id = self.conn.query_row(
            "SELECT id FROM apps WHERE bundle_id = ?1",
            [&app.bundle_id],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    pub fn get_all_apps(&self) -> AppResult<Vec<AppSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.bundle_id, a.display_name, a.app_path, a.installed_version,
                    a.install_source, a.is_ignored, a.icon_cache_path,
                    au.available_version, au.source_type,
                    a.homebrew_cask_token, a.sparkle_feed_url, a.obtained_from,
                    a.homebrew_formula_name,
                    au.release_notes, au.release_notes_url, au.notes,
                    a.description
             FROM apps a
             LEFT JOIN (
                 SELECT au1.* FROM available_updates au1
                 INNER JOIN (
                     SELECT app_id, MAX(detected_at) as max_detected
                     FROM available_updates
                     WHERE dismissed_at IS NULL
                     GROUP BY app_id
                 ) au2 ON au1.app_id = au2.app_id AND au1.detected_at = au2.max_detected
                 WHERE au1.dismissed_at IS NULL
             ) au ON au.app_id = a.id
                  AND (a.installed_version IS NULL OR au.available_version != a.installed_version)
             ORDER BY a.display_name COLLATE NOCASE",
        )?;

        let apps = stmt
            .query_map([], |row| {
                Ok(AppSummary {
                    id: row.get(0)?,
                    bundle_id: row.get(1)?,
                    display_name: row.get(2)?,
                    app_path: row.get(3)?,
                    installed_version: row.get(4)?,
                    install_source: row.get::<_, String>(5)?,
                    is_ignored: row.get::<_, i32>(6)? != 0,
                    icon_cache_path: row.get(7)?,
                    has_update: row.get::<_, Option<String>>(8)?.is_some(),
                    available_version: row.get(8)?,
                    update_source: row.get(9)?,
                    homebrew_cask_token: row.get(10)?,
                    sparkle_feed_url: row.get(11)?,
                    obtained_from: row.get(12)?,
                    homebrew_formula_name: row.get(13)?,
                    release_notes: row.get(14)?,
                    release_notes_url: row.get(15)?,
                    update_notes: row.get(16)?,
                    description: row.get(17)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(apps)
    }

    pub fn get_app_detail(&self, bundle_id: &str) -> AppResult<AppDetail> {
        let app = self.conn.query_row(
            "SELECT id, bundle_id, display_name, app_path, installed_version, bundle_version,
                    icon_cache_path, architectures, install_source, obtained_from,
                    homebrew_cask_token, is_ignored, first_seen_at, last_seen_at, mas_app_id,
                    homebrew_formula_name, description
             FROM apps WHERE bundle_id = ?1",
            [bundle_id],
            |row| {
                let arch_json: Option<String> = row.get(7)?;
                Ok(AppDetail {
                    id: row.get(0)?,
                    bundle_id: row.get(1)?,
                    display_name: row.get(2)?,
                    app_path: row.get(3)?,
                    installed_version: row.get(4)?,
                    bundle_version: row.get(5)?,
                    icon_cache_path: row.get(6)?,
                    architectures: arch_json.and_then(|j| serde_json::from_str(&j).ok()),
                    install_source: row.get(8)?,
                    obtained_from: row.get(9)?,
                    homebrew_cask_token: row.get(10)?,
                    is_ignored: row.get::<_, i32>(11)? != 0,
                    first_seen_at: row.get(12)?,
                    last_seen_at: row.get(13)?,
                    mas_app_id: row.get(14)?,
                    homebrew_formula_name: row.get(15)?,
                    description: row.get(16)?,
                    update_sources: Vec::new(),
                    available_update: None,
                })
            },
        )?;

        let mut sources_stmt = self.conn.prepare(
            "SELECT source_type, source_url, is_primary, last_checked_at
             FROM update_sources WHERE app_id = ?1",
        )?;
        let update_sources: Vec<UpdateSourceInfo> = sources_stmt
            .query_map([app.id], |row| {
                Ok(UpdateSourceInfo {
                    source_type: row.get(0)?,
                    source_url: row.get(1)?,
                    is_primary: row.get::<_, i32>(2)? != 0,
                    last_checked_at: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let available_update: Option<AvailableUpdateInfo> = self
            .conn
            .query_row(
                "SELECT available_version, source_type, release_notes_url, download_url,
                        release_notes, is_paid_upgrade, detected_at, notes
                 FROM available_updates
                 WHERE app_id = ?1 AND dismissed_at IS NULL
                 ORDER BY detected_at DESC LIMIT 1",
                [app.id],
                |row| {
                    Ok(AvailableUpdateInfo {
                        available_version: row.get(0)?,
                        source_type: row.get(1)?,
                        release_notes_url: row.get(2)?,
                        download_url: row.get(3)?,
                        release_notes: row.get(4)?,
                        is_paid_upgrade: row.get::<_, i32>(5)? != 0,
                        detected_at: row.get(6)?,
                        notes: row.get(7)?,
                    })
                },
            )
            .ok();

        Ok(AppDetail {
            update_sources,
            available_update,
            ..app
        })
    }

    pub fn set_app_ignored(&self, bundle_id: &str, ignored: bool) -> AppResult<()> {
        self.conn.execute(
            "UPDATE apps SET is_ignored = ?1 WHERE bundle_id = ?2",
            rusqlite::params![ignored as i32, bundle_id],
        )?;
        Ok(())
    }

    pub fn update_icon_cache_path(&self, bundle_id: &str, path: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE apps SET icon_cache_path = ?1 WHERE bundle_id = ?2",
            rusqlite::params![path, bundle_id],
        )?;
        Ok(())
    }

    pub fn update_cask_token(&self, bundle_id: &str, token: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE apps SET homebrew_cask_token = ?1 WHERE bundle_id = ?2 AND homebrew_cask_token IS NULL",
            rusqlite::params![token, bundle_id],
        )?;
        Ok(())
    }

    pub fn get_github_mappings(&self) -> HashMap<String, String> {
        let mut mappings = HashMap::new();
        let mut stmt = match self.conn.prepare(
            "SELECT bundle_id, github_repo FROM app_mappings WHERE github_repo IS NOT NULL",
        ) {
            Ok(s) => s,
            Err(_) => return mappings,
        };

        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                mappings.insert(row.0, row.1);
            }
        }

        mappings
    }

    pub fn update_installed_version(&self, app_id: i64, version: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE apps SET installed_version = ?1 WHERE id = ?2",
            rusqlite::params![version, app_id],
        )?;
        Ok(())
    }

    pub fn get_app_count(&self) -> AppResult<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM apps", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get the cached SHA-256 for a cask token (used for "latest" cask change detection).
    pub fn get_cask_sha(&self, cask_token: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT last_sha256 FROM cask_sha_cache WHERE cask_token = ?1",
                [cask_token],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn update_description(&self, app_id: i64, description: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE apps SET description = ?1 WHERE id = ?2",
            rusqlite::params![description, app_id],
        )?;
        Ok(())
    }

    /// Get apps that have a cask token but no description.
    /// Returns (app_id, cask_token, bundle_id, display_name).
    pub fn get_apps_missing_descriptions(&self) -> AppResult<Vec<(i64, Option<String>, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, homebrew_cask_token, bundle_id, display_name FROM apps WHERE description IS NULL",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Store or update the SHA-256 for a cask token.
    pub fn set_cask_sha(&self, cask_token: &str, sha256: &str) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO cask_sha_cache (cask_token, last_sha256, last_checked_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(cask_token) DO UPDATE SET
                last_sha256 = excluded.last_sha256,
                last_checked_at = datetime('now')",
            rusqlite::params![cask_token, sha256],
        )?;
        Ok(())
    }
}
