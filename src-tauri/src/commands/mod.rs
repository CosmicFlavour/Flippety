use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};

pub mod cards;
pub mod decks;
pub mod import_export;
pub mod study;

pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    /// Locks the database connection. Recovers from a poisoned lock (left
    /// behind by a panic in an earlier command) instead of panicking again,
    /// so one bad command can't brick every command after it for the rest
    /// of the session.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
