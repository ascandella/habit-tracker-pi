use crate::streak::StreakData;

#[derive(Debug, Clone)]
pub struct AccessLayer {
    conn: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
}

#[derive(thiserror::Error, Debug)]
pub enum DataAccessError {
    #[error("sqlite error")]
    SqliteError(#[from] rusqlite::Error),
    #[error("parse date error")]
    ParseDateError(#[from] chrono::ParseError),
    #[error("lock error")]
    LockError,
    #[error("too many references to drop")]
    TooManyReferencesToDrop,
}

const FETCH_SIZE: usize = 100;
type UtcDateTime = chrono::DateTime<chrono::Utc>;

impl AccessLayer {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self {
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    pub fn record_event(&self) -> Result<(), DataAccessError> {
        let now: UtcDateTime = chrono::Utc::now();
        self.record_event_at(&now)
    }

    pub(crate) fn record_event_at(&self, time: &UtcDateTime) -> Result<(), DataAccessError> {
        self.lock_conn()?.execute(
            "INSERT INTO events (timestamp) VALUES (?1)",
            [sqlite_datetime(time)],
        )?;
        Ok(())
    }

    pub fn current_streak(
        &self,
        timezone: &impl chrono::TimeZone,
    ) -> Result<StreakData, DataAccessError> {
        // In case an event was just recorded, we use exclusive date boundaries
        // in our streak comparison and millisecond precision.
        let upper_bound = chrono::Utc::now() + chrono::Duration::seconds(1);
        self.streak_from_time(timezone, &upper_bound, false)
    }

    pub fn previous_streak(
        &self,
        timezone: &impl chrono::TimeZone,
        streak_data: &StreakData,
    ) -> Result<StreakData, DataAccessError> {
        let upper_bound = match streak_data {
            StreakData::NoData => &chrono::Utc::now(),
            StreakData::Streak(streak) => streak.start(),
        };
        self.streak_from_time(timezone, upper_bound, true)
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<rusqlite::Connection>, DataAccessError> {
        self.conn.lock().map_err(|_| DataAccessError::LockError)
    }

    #[tracing::instrument(skip(self, timezone))]
    fn streak_from_time(
        &self,
        timezone: &impl chrono::TimeZone,
        end: &UtcDateTime,
        allow_gap: bool,
    ) -> Result<StreakData, DataAccessError> {
        let mut streak_alive = true;
        let mut streak_end = *end;
        let mut dates = vec![];

        while streak_alive {
            let conn = self.lock_conn()?;
            // Return the current streak, based on querying the events table
            let mut stmt = conn.prepare(
                r#"
                    SELECT timestamp FROM events
                    WHERE timestamp < ?1
                    ORDER BY timestamp DESC LIMIT ?2
                "#,
            )?;
            let rows = stmt
                .query_map(
                    [sqlite_datetime(&streak_end), FETCH_SIZE.to_string()],
                    |row| {
                        let timestamp: String = row.get(0)?;
                        Ok(timestamp)
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;

            if rows.is_empty() {
                // Base case: no more rows returned, we're done searching
                break;
            }

            for timestamp in &rows {
                let parsed_timestamp =
                    UtcDateTime::from(chrono::DateTime::parse_from_rfc3339(timestamp)?);

                if allow_gap && dates.is_empty() {
                    // For "previous streak" logic, just pick the first date we find, no need to
                    // compare to anything
                    dates.push(parsed_timestamp);
                } else {
                    let end_comparison = dates.last().unwrap_or(&streak_end);

                    // If the date we're looking at is the same day as the most recent one
                    // we found, or exactly 1 day behind (in the provided timezone), the
                    // streak is alive.
                    if days_between(timezone, &parsed_timestamp, end_comparison) <= 1 {
                        dates.push(parsed_timestamp);
                    } else {
                        // Otherwise, it's been too long and the streak is broken
                        streak_alive = false;
                        break;
                    }
                }
            }

            // If we have found a date that's part of the streak, the oldest (end of the
            // list, aka most recently pushed on) is now the date we're comparing against to
            // keep the streak alive.
            if let Some(date) = dates.last() {
                streak_end = *date
            }
        }

        Ok(dates.into())
    }

    pub fn close(self) -> Result<(), DataAccessError> {
        let inner_mutex = std::sync::Arc::into_inner(self.conn)
            .ok_or(DataAccessError::TooManyReferencesToDrop)?;

        inner_mutex
            .into_inner()
            .map_err(|_| DataAccessError::LockError)?
            .close()
            .map_err(|(_, e)| e)?;
        Ok(())
    }
}

pub(crate) fn days_between(
    timezone: &impl chrono::TimeZone,
    first: &UtcDateTime,
    second: &UtcDateTime,
) -> i64 {
    let first = first.with_timezone(timezone).date_naive();
    let second = second.with_timezone(timezone).date_naive();
    (first - second).abs().num_days()
}

fn sqlite_datetime(time: &UtcDateTime) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::migrations;

    fn create_access() -> AccessLayer {
        let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory");
        migrations::migrate(&mut conn).expect("migrate");
        AccessLayer::new(conn)
    }

    #[test]
    fn test_record_event_ok() {
        let db = create_access();
        let test_resp = db.record_event();
        assert!(test_resp.is_ok());
    }

    #[test]
    fn test_multiple_closes_error() {
        let db = create_access();
        let cloned = db.clone();

        match cloned.close() {
            Ok(_) => panic!("expected error"),
            Err(err) => {
                assert!(matches!(err, DataAccessError::TooManyReferencesToDrop));
            }
        }

        assert!(db.close().is_ok());
    }

    #[test]
    fn test_record_event_multiple_threads() {
        let db = create_access();
        let cloned = db.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            cloned.record_event().expect("record event");
            tx.send(()).expect("send done signal");
        });
        rx.recv().expect("receive");

        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");

        match streak {
            StreakData::Streak(streak) => {
                assert_eq!(streak.count(), 1);
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn test_close() {
        let db = create_access();
        assert!(db.close().is_ok());
    }

    #[test]
    fn test_sqlite_datetime_formatting() {
        let dt: UtcDateTime = chrono::Utc
            .with_ymd_and_hms(2024, 7, 21, 15, 30, 0)
            .unwrap();
        let time_str = sqlite_datetime(&dt);
        assert_eq!(time_str, "2024-07-21T15:30:00.000Z");
    }

    #[test]
    fn test_streak_no_data() {
        let db = create_access();
        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");
        assert!(matches!(streak, StreakData::NoData));
        let streak = db
            .previous_streak(&chrono::Utc, &streak)
            .expect("fetch previous streak");
        assert!(matches!(streak, StreakData::NoData));
    }

    #[test]
    fn test_streak_few_days_ago() {
        let db = create_access();
        let then = chrono::Utc::now() - chrono::Duration::days(3);
        db.record_event_at(&then).expect("record event");
        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");
        assert!(matches!(streak, StreakData::NoData));

        let previous_streak = db
            .previous_streak(&chrono::Utc, &streak)
            .expect("fetch previous streak");

        match previous_streak {
            StreakData::Streak(ref streak) => {
                assert_eq!(streak.count(), 1);
                assert_eq!(streak.days(&chrono::Utc), 1);
            }
            _ => panic!("expected streak"),
        }

        let previous_streak = db
            .previous_streak(&chrono::Utc, &previous_streak)
            .expect("fetch previous streak");
        assert!(matches!(previous_streak, StreakData::NoData));
    }

    #[test]
    fn test_streak_one_day() {
        let db = create_access();
        db.record_event().expect("record event");

        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");

        match streak {
            StreakData::Streak(streak) => {
                assert_eq!(streak.count(), 1);
                assert_eq!(streak.days(&chrono::Utc), 1);
                assert_eq!(streak.start().date_naive(), chrono::Utc::now().date_naive());
                assert_eq!(streak.end().date_naive(), chrono::Utc::now().date_naive());
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn test_streak_three_days() {
        let db = create_access();
        let now = chrono::Utc::now();
        let dates = vec![
            now,
            now - chrono::Duration::days(1),
            now - chrono::Duration::days(2),
            // Gap here, streak ends
            now - chrono::Duration::days(4),
            now - chrono::Duration::days(5),
        ];
        for date in dates {
            db.record_event_at(&date).expect("record event");
        }

        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");

        match streak {
            StreakData::Streak(ref streak) => {
                assert_eq!(streak.count(), 3);
                assert_eq!(streak.days(&chrono::Utc), 3);
                assert_eq!(streak.end().date_naive(), now.date_naive());
                assert_eq!(
                    streak.start().date_naive(),
                    (now - chrono::Duration::days(2)).date_naive(),
                );
            }
            _ => panic!("expected streak"),
        }

        let previous_streak = db
            .previous_streak(&chrono::Utc, &streak)
            .expect("fetch previous streak");

        match previous_streak {
            StreakData::Streak(ref streak) => {
                assert_eq!(streak.count(), 2);
                assert_eq!(streak.days(&chrono::Utc), 2);
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn gap_in_previous() {
        let db = create_access();
        let now = chrono::Utc::now();
        let times = vec![
            chrono::Duration::days(1),
            chrono::Duration::days(2),
            chrono::Duration::days(8),
            chrono::Duration::days(9),
            chrono::Duration::days(10),
            chrono::Duration::days(12),
        ];
        for time in times {
            db.record_event_at(&(now - time)).expect("record event");
        }

        let streak = db
            .streak_from_time(&chrono::Utc, &now, false)
            .expect("fetch current streak");
        assert!(matches!(streak, StreakData::Streak(_)));

        match db
            .previous_streak(&chrono::Utc, &streak)
            .expect("fetch previous streak")
        {
            StreakData::Streak(ref streak) => {
                assert_eq!(streak.days(&chrono::Utc), 3);
            }
            StreakData::NoData => panic!("expected streak"),
        }
    }

    #[test]
    fn test_streak_real_data() {
        let db = create_access();
        let times = vec![
            "2024-07-26T23:40:03.405Z",
            "2024-07-25T20:36:21.789Z",
            "2024-07-24T15:03:39.952Z",
            "2024-07-23T15:03:39.952Z",
        ];
        for time in &times {
            let dt = UtcDateTime::from(chrono::DateTime::parse_from_rfc3339(time).unwrap());
            db.record_event_at(&dt).expect("record event");
        }
        let now = UtcDateTime::from(
            chrono::DateTime::parse_from_rfc3339("2024-07-26T23:40:04.405Z").unwrap(),
        );
        let pacific = chrono_tz::US::Pacific;

        let streak = db
            .streak_from_time(&pacific, &now, false)
            .expect("fetch current streak");
        match streak {
            StreakData::Streak(ref streak) => {
                assert_eq!(streak.count(), times.len());
                assert_eq!(streak.days(&pacific), 4);
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn test_streak_multiple_queries() {
        let db = create_access();
        let now = chrono::Utc::now();

        for days in 0..FETCH_SIZE + 1 {
            db.record_event_at(&(now - chrono::Duration::days(days as i64)))
                .expect("record event");
        }

        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");

        match streak {
            StreakData::Streak(streak) => {
                assert_eq!(streak.count(), FETCH_SIZE + 1);
                assert_eq!(
                    streak.days(&chrono::Utc),
                    (FETCH_SIZE + 1).try_into().unwrap()
                );
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn test_days_between() {
        let pacific = chrono_tz::US::Pacific;
        let dt: UtcDateTime = chrono::Utc
            .with_ymd_and_hms(2024, 7, 21, 23, 30, 0)
            .unwrap();
        let yesterday = dt - chrono::Duration::days(1);
        assert_eq!(0, days_between(&pacific, &dt, &dt));
        assert_eq!(1, days_between(&pacific, &dt, &yesterday));
        assert_eq!(1, days_between(&pacific, &yesterday, &dt));

        let beginning_of_previous_day_pacific = chrono::Utc
            .with_ymd_and_hms(2024, 7, 20, 13, 30, 0)
            .unwrap();

        assert_eq!(
            1,
            days_between(&pacific, &beginning_of_previous_day_pacific, &dt)
        );

        let beginning_of_previous_day_utc =
            chrono::Utc.with_ymd_and_hms(2024, 7, 20, 3, 30, 0).unwrap();

        assert_eq!(
            2,
            days_between(&pacific, &beginning_of_previous_day_utc, &dt)
        );

        let eod_pacific = chrono::Utc
            .with_ymd_and_hms(2024, 7, 22, 0, 59, 59)
            .unwrap();
        let soprevious_pacific = chrono::Utc.with_ymd_and_hms(2024, 7, 20, 8, 0, 0).unwrap();

        assert_eq!(1, days_between(&pacific, &eod_pacific, &soprevious_pacific));
        assert_eq!(
            2,
            days_between(&chrono::Utc, &eod_pacific, &soprevious_pacific)
        );
    }
}
