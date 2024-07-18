use std::path::Path;

use thiserror::Error;

pub(crate) mod access_layer;
pub(crate) mod migrations;
pub use access_layer::{AccessLayer, DataAccessError};

#[derive(Error, Debug)]
pub enum DbError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
    #[error("migration error")]
    MigrationError(#[from] rusqlite_migration::Error),
    #[error("data access error")]
    DataAccessError(#[from] DataAccessError),
}

pub fn in_memory() -> Result<AccessLayer, DbError> {
    let mut conn = rusqlite::Connection::open_in_memory()?;
    migrations::migrate(&mut conn)?;
    Ok(AccessLayer::new(conn))
}

pub fn open_file(path: impl AsRef<Path>) -> Result<AccessLayer, DbError> {
    let mut conn = rusqlite::Connection::open(path)?;

    // Apply some PRAGMA, often better to do it outside of migrations
    conn.pragma_update_and_check(None, "journal_mode", &"WAL", |_| Ok(()))?;

    migrations::migrate(&mut conn)?;
    Ok(AccessLayer::new(conn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

    #[test]
    fn test_in_memory() {
        assert!(in_memory().is_ok());
    }

    #[test]
    fn test_open_file() {
        let file = tempfile::NamedTempFile::new().expect("create temp file");
        assert!(open_file(file.path()).is_ok());
    }
}
