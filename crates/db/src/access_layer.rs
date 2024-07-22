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

pub enum StreakData {
    NoData,
    Streak(Streak),
}

impl From<Vec<chrono::DateTime<chrono::Utc>>> for StreakData {
    fn from(times: Vec<chrono::DateTime<chrono::Utc>>) -> Self {
        if times.is_empty() {
            StreakData::NoData
        } else {
            StreakData::Streak(Streak::new(times))
        }
    }
}

#[derive(Debug)]
pub struct Streak {
    // Stored in reverse order, where the first element of the list has the newest (most
    // recent) date of the streak. The last element will be the end of the streak.
    times: Vec<chrono::DateTime<chrono::Utc>>,
}

impl Streak {
    fn new(times: Vec<chrono::DateTime<chrono::Utc>>) -> Self {
        assert!(!times.is_empty());
        Self { times }
    }

    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn start(&self) -> &chrono::DateTime<chrono::Utc> {
        self.times
            .last()
            .expect("invariant violation: times must be non-empty")
    }

    pub fn end(&self) -> &chrono::DateTime<chrono::Utc> {
        self.times
            .first()
            .expect("invariant violation: times must be non-empty")
    }
}

impl AccessLayer {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn record_event(&self) -> Result<(), DataAccessError> {
        let now = std::time::SystemTime::now();
        let now: chrono::DateTime<chrono::Utc> = now.into();
        self.record_event_at(&now)
    }

    pub(crate) fn record_event_at(
        &self,
        time: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DataAccessError> {
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
        let now = chrono::Utc::now() + chrono::Duration::seconds(1);
        self.streak_from_time(timezone, &now)
    }

    fn streak_from_time(
        &self,
        timezone: &impl chrono::TimeZone,
        end: &chrono::DateTime<chrono::Utc>,
    ) -> Result<StreakData, DataAccessError> {
        let fetch_size: u32 = 100;
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
                    [sqlite_datetime(&streak_end), fetch_size.to_string()],
                    |row| {
                        let timestamp: String = row.get(0)?;
                        Ok(timestamp)
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|timestamp| {
                    let res = chrono::DateTime::parse_from_rfc3339(&timestamp)?;
                    Ok::<_, chrono::ParseError>(chrono::DateTime::<chrono::Utc>::from(res))
                })
                .collect::<Result<Vec<_>, _>>()?;

            if rows.is_empty() {
                break;
            }

            for &timestamp in &rows {
                let end_comparison = dates.first().unwrap_or(&streak_end);

                if is_previous_or_same_day(timezone, &timestamp, end_comparison) {
                    dates.push(timestamp);
                } else {
                    streak_alive = false;
                    break;
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
    first: &chrono::DateTime<chrono::Utc>,
    second: &chrono::DateTime<chrono::Utc>,
) -> bool {
    let first = first.with_timezone(timezone).date_naive();
    let second = second.with_timezone(timezone).date_naive();
    first == second || ((first - second).abs() == chrono::TimeDelta::days(1))
}

fn sqlite_datetime(time: &chrono::DateTime<chrono::Utc>) -> String {
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
    fn test_access_layer() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory");
        migrations::migrate(&mut conn).expect("migrate");
        let access_layer = AccessLayer::new(conn);
        let test_resp = access_layer.record_event();
        assert!(test_resp.is_ok());
    }

    #[test]
    fn test_sqlite_datetime() {
        let dt: chrono::DateTime<chrono::Utc> = chrono::Utc
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
    fn test_streak_two_days() {
        let db = create_access();
        let now = chrono::Utc::now();
        let dates = vec![
            now,
            now - chrono::Duration::days(1),
            // Gap here, streak ends
            now - chrono::Duration::days(3),
            now - chrono::Duration::days(4),
        ];
        for date in dates {
            db.record_event_at(&date).expect("record event");
        }

        let streak = db
            .current_streak(&chrono::Utc)
            .expect("fetch current streak");

        match streak {
            StreakData::Streak(streak) => {
                assert_eq!(streak.len(), 2);
                assert_eq!(streak.end().date_naive(), now.date_naive());
                assert_eq!(
                    streak.start().date_naive(),
                    (now - chrono::Duration::days(1)).date_naive(),
                );
            }
            _ => panic!("expected streak"),
        }
    }

    #[test]
    fn test_is_same_day_timezone() {
        let timezone = chrono_tz::US::Pacific;
        let dt: chrono::DateTime<chrono::Utc> = chrono::Utc
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
