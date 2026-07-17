//! Filesystem preparation for the SQLite adapter.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};

const CURRENT_SCHEMA_VERSION: i64 = 2;

pub(crate) fn prepare_database_path(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建 db 父目录失败 {}", parent.display()))?;
        }
    }
    backup_before_migration(path)
}

fn backup_before_migration(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let connection = Connection::open(path)?;
    let current = connection
        .query_row(
            "SELECT version FROM schema_version WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .unwrap_or(None);
    if current.unwrap_or(0) >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    let backup_path = path.with_extension("db.pre-migration.bak");
    fs::copy(path, &backup_path).with_context(|| {
        format!(
            "create database migration backup failed {}",
            backup_path.display()
        )
    })?;
    Ok(())
}
