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

    pub fn test(&self) -> Result<usize, DataAccessError> {
        let mut stmt = self.conn.prepare("SELECT * FROM test")?;
        let res = stmt
            .query_map([], |_| Ok(()))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(res.len())
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
        let test_resp = access_layer.test();
        assert!(test_resp.is_ok());
        assert!(test_resp.unwrap() == 0);
    }
}
