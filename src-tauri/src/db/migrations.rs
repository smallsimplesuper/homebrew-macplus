use crate::db::Database;
use crate::utils::AppResult;

const MIGRATIONS: &[&str] = &[
    // Migration 1: Initial schema
    "
    CREATE TABLE IF NOT EXISTS apps (
        id                  INTEGER PRIMARY KEY AUTOINCREMENT,
        bundle_id           TEXT NOT NULL UNIQUE,
        display_name        TEXT NOT NULL,
        app_path            TEXT NOT NULL,
        installed_version   TEXT,
        bundle_version      TEXT,
        icon_cache_path     TEXT,
        architectures       TEXT,
        install_source      TEXT DEFAULT 'unknown',
        obtained_from       TEXT,
        homebrew_cask_token TEXT,
        is_ignored          INTEGER DEFAULT 0,
        first_seen_at       TEXT DEFAULT (datetime('now')),
        last_seen_at        TEXT DEFAULT (datetime('now')),
        last_scanned_at     TEXT
    );

    CREATE TABLE IF NOT EXISTS update_sources (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id          INTEGER REFERENCES apps(id) ON DELETE CASCADE,
        source_type     TEXT NOT NULL,
        source_url      TEXT,
        is_primary      INTEGER DEFAULT 0,
        last_checked_at TEXT,
        UNIQUE(app_id, source_type)
    );

    CREATE TABLE IF NOT EXISTS available_updates (
        id                  INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id              INTEGER REFERENCES apps(id) ON DELETE CASCADE,
        source_type         TEXT NOT NULL,
        available_version   TEXT NOT NULL,
        release_notes_url   TEXT,
        download_url        TEXT,
        is_paid_upgrade     INTEGER DEFAULT 0,
        detected_at         TEXT DEFAULT (datetime('now')),
        dismissed_at        TEXT,
        UNIQUE(app_id, available_version)
    );

    CREATE TABLE IF NOT EXISTS update_history (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id          INTEGER REFERENCES apps(id) ON DELETE CASCADE,
        from_version    TEXT NOT NULL,
        to_version      TEXT NOT NULL,
        source_type     TEXT NOT NULL,
        status          TEXT DEFAULT 'pending',
        error_message   TEXT,
        started_at      TEXT,
        completed_at    TEXT
    );

    CREATE TABLE IF NOT EXISTS settings (
        key         TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        updated_at  TEXT DEFAULT (datetime('now'))
    );

    CREATE TABLE IF NOT EXISTS app_mappings (
        bundle_id           TEXT PRIMARY KEY,
        homebrew_cask_token TEXT,
        github_repo         TEXT,
        mas_id              INTEGER,
        custom_feed_url     TEXT,
        is_user_defined     INTEGER DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_apps_bundle_id ON apps(bundle_id);
    CREATE INDEX IF NOT EXISTS idx_available_updates_app_id ON available_updates(app_id);
    CREATE INDEX IF NOT EXISTS idx_update_history_app_id ON update_history(app_id);
    ",
    // Migration 2: Add sparkle_feed_url and mas_app_id columns
    "
    ALTER TABLE apps ADD COLUMN sparkle_feed_url TEXT;
    ALTER TABLE apps ADD COLUMN mas_app_id TEXT;
    ",
    // Migration 3: Add release_notes column to available_updates
    "
    ALTER TABLE available_updates ADD COLUMN release_notes TEXT;
    ",
    // Migration 4: Add homebrew_formula_name column
    "
    ALTER TABLE apps ADD COLUMN homebrew_formula_name TEXT;
    ",
    // Migration 5: Add notes column to available_updates for prerequisite warnings
    "
    ALTER TABLE available_updates ADD COLUMN notes TEXT;
    ",
    // Migration 6: Add indexes for cask-token lookups and dismissed update filtering
    "
    CREATE INDEX IF NOT EXISTS idx_apps_homebrew_cask_token ON apps(homebrew_cask_token);
    CREATE INDEX IF NOT EXISTS idx_available_updates_dismissed ON available_updates(dismissed_at);
    ",
    // Migration 7: Add cask_sha_cache table for SHA-256 change detection on "latest" casks
    "
    CREATE TABLE IF NOT EXISTS cask_sha_cache (
        cask_token      TEXT PRIMARY KEY,
        last_sha256     TEXT NOT NULL,
        last_checked_at TEXT DEFAULT (datetime('now'))
    );
    ",
    // Migration 8: Add indexes for frequently queried columns
    "
    CREATE INDEX IF NOT EXISTS idx_apps_install_source ON apps(install_source);
    CREATE INDEX IF NOT EXISTS idx_available_updates_app_dismissed ON available_updates(app_id, dismissed_at);
    CREATE INDEX IF NOT EXISTS idx_update_history_status ON update_history(status);
    ",
];

pub fn run_migrations(db: &mut Database) -> AppResult<()> {
    db.conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY,
            applied_at TEXT DEFAULT (datetime('now'))
        );",
    )?;

    let applied: i64 = db
        .conn
        .query_row("SELECT COALESCE(MAX(id), 0) FROM _migrations", [], |row| {
            row.get(0)
        })?;

    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > applied {
            db.conn.execute_batch(migration)?;
            db.conn.execute(
                "INSERT INTO _migrations (id) VALUES (?1)",
                [version],
            )?;
            log::info!("Applied migration {}", version);
        }
    }

    Ok(())
}
