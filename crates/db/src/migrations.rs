use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

pub(crate) fn migrate(conn: &mut Connection) -> rusqlite_migration::Result<()> {
    let migrations = Migrations::new(vec![
        M::up(
            r#"CREATE TABLE events (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        );"#,
        )
        .down("DROP TABLE events;"),
        M::up("CREATE INDEX idx_events_timestamp ON events (timestamp);")
            .down("DROP INDEX idx_events_timestamp"),
    ]);
    migrations.to_latest(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_migrate() {
        let mut conn = Connection::open_in_memory().expect("create in-memory");
        assert!(migrate(&mut conn).is_ok());
    }
}
