use crate::db::Database;
use crate::models::UpdateHistoryEntry;
use crate::utils::AppResult;

impl Database {
    pub fn get_update_history(&self, limit: i64) -> AppResult<Vec<UpdateHistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT h.id, a.bundle_id, a.display_name, a.icon_cache_path,
                    h.from_version, h.to_version, h.source_type,
                    h.status, h.error_message, h.started_at, h.completed_at
             FROM update_history h
             JOIN apps a ON a.id = h.app_id
             ORDER BY h.started_at DESC
             LIMIT ?1",
        )?;

        let entries = stmt
            .query_map([limit], |row| {
                Ok(UpdateHistoryEntry {
                    id: row.get(0)?,
                    bundle_id: row.get(1)?,
                    display_name: row.get(2)?,
                    icon_cache_path: row.get(3)?,
                    from_version: row.get(4)?,
                    to_version: row.get(5)?,
                    source_type: row.get(6)?,
                    status: row.get(7)?,
                    error_message: row.get(8)?,
                    started_at: row.get(9)?,
                    completed_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    pub fn record_update_start(
        &self,
        app_id: i64,
        from_version: &str,
        to_version: &str,
        source_type: &str,
    ) -> AppResult<i64> {
        self.conn.execute(
            "INSERT INTO update_history (app_id, from_version, to_version, source_type, status, started_at)
             VALUES (?1, ?2, ?3, ?4, 'in_progress', datetime('now'))",
            rusqlite::params![app_id, from_version, to_version, source_type],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn record_update_complete(&self, history_id: i64) -> AppResult<()> {
        self.conn.execute(
            "UPDATE update_history SET status = 'completed', completed_at = datetime('now')
             WHERE id = ?1",
            [history_id],
        )?;
        Ok(())
    }

    pub fn record_update_delegated(&self, history_id: i64) -> AppResult<()> {
        self.conn.execute(
            "UPDATE update_history SET status = 'delegated', completed_at = datetime('now')
             WHERE id = ?1",
            [history_id],
        )?;
        Ok(())
    }

    pub fn record_update_failed(&self, history_id: i64, error: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE update_history SET status = 'failed', error_message = ?1, completed_at = datetime('now')
             WHERE id = ?2",
            rusqlite::params![error, history_id],
        )?;
        Ok(())
    }
}
