use crate::streak::StreakData;

#[derive(Debug)]
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

const FETCH_SIZE: usize = 100;
type UtcDateTime = chrono::DateTime<chrono::Utc>;

impl AccessLayer {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn record_event(&self) -> Result<(), DataAccessError> {
        let now: UtcDateTime = chrono::Utc::now();
        self.record_event_at(&now)
    }

    pub(crate) fn record_event_at(&self, time: &UtcDateTime) -> Result<(), DataAccessError> {
        self.conn.execute(
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

    #[tracing::instrument(skip(timezone))]
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
            // Return the current streak, based on querying the events table
            let mut stmt = self.conn.prepare(
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

                    if is_previous_or_same_day(timezone, &parsed_timestamp, end_comparison) {
                        dates.push(parsed_timestamp);
                    } else {
                        streak_alive = false;
                        break;
                    }
                }
            }

            if let Some(date) = dates.last() {
                streak_end = *date
            }
        }

        Ok(StreakData::from(dates))
    }
}

fn is_previous_or_same_day(
    timezone: &impl chrono::TimeZone,
    first: &UtcDateTime,
    second: &UtcDateTime,
) -> bool {
    let first = first.with_timezone(timezone).date_naive();
    let second = second.with_timezone(timezone).date_naive();
    (first - second).abs() <= chrono::TimeDelta::days(1)
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
                assert_eq!(streak.len(), 1);
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
                assert_eq!(streak.len(), 1);
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
                assert_eq!(streak.len(), 3);
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
                assert_eq!(streak.len(), 2);
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
                assert_eq!(streak.len(), FETCH_SIZE + 1);
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn test_is_same_day_timezone() {
        let timezone = chrono_tz::US::Pacific;
        let dt: UtcDateTime = chrono::Utc
            .with_ymd_and_hms(2024, 7, 21, 23, 30, 0)
            .unwrap();
        let yesterday = dt - chrono::Duration::days(1);
        assert!(is_previous_or_same_day(&timezone, &dt, &dt));
        assert!(is_previous_or_same_day(&timezone, &dt, &yesterday));
        assert!(is_previous_or_same_day(&timezone, &yesterday, &dt));

        let beginning_of_previous_day_pacific = chrono::Utc
            .with_ymd_and_hms(2024, 7, 20, 13, 30, 0)
            .unwrap();

        assert!(is_previous_or_same_day(
            &timezone,
            &beginning_of_previous_day_pacific,
            &dt
        ));

        let beginning_of_previous_day_utc =
            chrono::Utc.with_ymd_and_hms(2024, 7, 20, 3, 30, 0).unwrap();

        assert!(!is_previous_or_same_day(
            &timezone,
            &beginning_of_previous_day_utc,
            &dt
        ));

        let eod_pacific = chrono::Utc
            .with_ymd_and_hms(2024, 7, 22, 0, 59, 59)
            .unwrap();
        let soprevious_pacific = chrono::Utc.with_ymd_and_hms(2024, 7, 20, 8, 0, 0).unwrap();

        assert!(is_previous_or_same_day(
            &timezone,
            &eod_pacific,
            &soprevious_pacific,
        ));
        assert!(!is_previous_or_same_day(
            &chrono::Utc,
            &eod_pacific,
            &soprevious_pacific,
        ));
    }
}
