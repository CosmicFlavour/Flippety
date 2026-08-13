use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

pub mod cards;
pub mod decks;
pub mod review_state;

const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_init.sql"),
    include_str!("migrations/0002_leveling.sql"),
];

pub fn open(db_path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let current_version = current_version as usize;

    for (i, migration) in MIGRATIONS.iter().enumerate().skip(current_version) {
        conn.execute_batch(migration)?;
        conn.pragma_update(None, "user_version", (i + 1) as i64)?;
    }

    Ok(())
}

/// In-memory, fully-migrated connection for unit tests.
#[cfg(test)]
pub fn test_connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrate(&conn).unwrap();
    conn
}
