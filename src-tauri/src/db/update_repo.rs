use crate::db::Database;
use crate::models::UpdateInfo;
use crate::utils::AppResult;

impl Database {
    pub fn upsert_update_source(
        &self,
        app_id: i64,
        source_type: &str,
        source_url: Option<&str>,
        is_primary: bool,
    ) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO update_sources (app_id, source_type, source_url, is_primary, last_checked_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(app_id, source_type) DO UPDATE SET
                source_url = COALESCE(excluded.source_url, update_sources.source_url),
                is_primary = excluded.is_primary,
                last_checked_at = datetime('now')",
            rusqlite::params![app_id, source_type, source_url, is_primary as i32],
        )?;
        Ok(())
    }

    pub fn upsert_available_update(&self, app_id: i64, update: &UpdateInfo) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO available_updates (app_id, source_type, available_version, release_notes_url, download_url, release_notes, is_paid_upgrade, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(app_id, available_version) DO UPDATE SET
                source_type = excluded.source_type,
                release_notes_url = COALESCE(excluded.release_notes_url, available_updates.release_notes_url),
                download_url = COALESCE(excluded.download_url, available_updates.download_url),
                release_notes = COALESCE(excluded.release_notes, available_updates.release_notes),
                is_paid_upgrade = excluded.is_paid_upgrade,
                notes = excluded.notes",
            rusqlite::params![
                app_id,
                update.source_type.as_str(),
                update.available_version,
                update.release_notes_url,
                update.download_url,
                update.release_notes,
                update.is_paid_upgrade as i32,
                update.notes,
            ],
        )?;
        Ok(())
    }

    pub fn clear_available_updates(&self, app_id: i64) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM available_updates WHERE app_id = ?1",
            [app_id],
        )?;
        Ok(())
    }

    pub fn clear_updates_for_cask_token(&self, cask_token: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM available_updates WHERE app_id IN (
                SELECT id FROM apps WHERE homebrew_cask_token = ?1
            )",
            [cask_token],
        )?;
        Ok(())
    }

    pub fn dismiss_update(&self, app_id: i64, version: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE available_updates SET dismissed_at = datetime('now')
             WHERE app_id = ?1 AND available_version = ?2",
            rusqlite::params![app_id, version],
        )?;
        Ok(())
    }

    pub fn get_update_count(&self) -> AppResult<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT au.app_id) FROM available_updates au
             JOIN apps a ON a.id = au.app_id
             WHERE au.dismissed_at IS NULL AND a.is_ignored = 0
               AND (a.installed_version IS NULL OR au.available_version != a.installed_version)",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}
