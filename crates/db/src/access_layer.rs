pub struct AccessLayer {
    conn: rusqlite::Connection,
}

#[derive(thiserror::Error, Debug)]
pub enum DataAccessError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
}

impl AccessLayer {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn record_event(&self) -> Result<(), DataAccessError> {
        let now = std::time::SystemTime::now();
        let now: chrono::DateTime<chrono::Utc> = now.into();
        self.conn.execute(
            "INSERT INTO events (timestamp) VALUES (?1)",
            [now.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    #[test]
    fn test_access_layer() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory");
        migrations::migrate(&mut conn).expect("migrate");
        let access_layer = AccessLayer::new(conn);
        let test_resp = access_layer.record_event();
        assert!(test_resp.is_ok());
    }
}
