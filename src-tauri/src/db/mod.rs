pub mod app_repo;
pub mod history_repo;
pub mod migrations;
pub mod update_repo;

use rusqlite::Connection;
use std::path::Path;

use crate::utils::AppResult;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn new(db_path: &Path) -> AppResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let mut db = Self { conn };
        migrations::run_migrations(&mut db)?;

        // Purge stale update records where available == installed version
        let purged: usize = match db.conn.execute(
            "DELETE FROM available_updates WHERE id IN (
                SELECT au.id FROM available_updates au
                JOIN apps a ON a.id = au.app_id
                WHERE au.available_version = a.installed_version
            )",
            [],
        ) {
            Ok(count) => count,
            Err(e) => {
                log::warn!("Failed to purge stale updates at startup: {}", e);
                0
            }
        };
        if purged > 0 {
            log::info!("Purged {} stale update records (available == installed)", purged);
        }

        Ok(db)
    }
}
