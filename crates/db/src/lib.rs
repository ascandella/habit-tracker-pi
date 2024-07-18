use thiserror::Error;

pub(crate) mod migrations;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
    #[error("migration error")]
    MigrationError(#[from] rusqlite_migration::Error),
}

pub fn in_memory() -> Result<(), DbError> {
    let mut conn = rusqlite::Connection::open_in_memory()?;
    migrations::migrate(&mut conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory() {
        assert!(in_memory().is_ok());
    }
}
