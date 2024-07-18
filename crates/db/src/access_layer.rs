pub struct AccessLayer {
    conn: rusqlite::Connection,
}

#[derive(thiserror::Error, Debug)]
pub enum DataAccessError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
    #[error("parse date error")]
    ParseDateError(#[from] chrono::ParseError),
}

pub struct Streak {
    start: chrono::DateTime<chrono::Utc>,
    count: usize,
    times: Vec<chrono::DateTime<chrono::Utc>>,
    end: Option<chrono::DateTime<chrono::Utc>>,
}

impl AccessLayer {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn record_event(&self) -> Result<(), DataAccessError> {
        let now = std::time::SystemTime::now();
        let now: chrono::DateTime<chrono::Utc> = now.into();
        self.record_event_at(now)
    }

    pub(crate) fn record_event_at(
        &self,
        time: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DataAccessError> {
        self.conn.execute(
            "INSERT INTO events (timestamp) VALUES (?1)",
            [time.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)],
        )?;
        Ok(())
    }

    pub fn current_streak(&self, timezone: impl chrono::TimeZone) -> Result<(), DataAccessError> {
        let fetch_size: u32 = 10;
        let mut streak_alive = true;
        let mut streak_end: Option<chrono::DateTime<chrono::Utc>> = None;

        while streak_alive {
            // Return the current streak, based on querying the events table
            let mut stmt = self
                .conn
                .prepare("SELECT * timestamp FROM events ORDER BY timestamp DESC LIMIT ?1")?;
            let rows = stmt
                .query_map([fetch_size], |row| {
                    let timestamp: String = row.get(0)?;
                    Ok(timestamp)
                })?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|timestamp| {
                    let res = chrono::DateTime::parse_from_rfc3339(&timestamp)?;
                    Ok::<_, chrono::ParseError>(chrono::DateTime::<chrono::Utc>::from(res))
                })
                .collect::<Result<Vec<_>, _>>()?;
        }
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
