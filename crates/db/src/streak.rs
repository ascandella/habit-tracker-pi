#[derive(Debug)]
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

    /// Total number of events in the streak. Will always be less than or equal to `days()`
    pub fn count(&self) -> usize {
        self.times.len()
    }

    /// Total number of days the streak was alive for
    pub fn days(&self, timezone: &impl chrono::TimeZone) -> i64 {
        super::access_layer::days_between(timezone, self.start(), self.end()) + 1
    }

    /// When the streak started
    pub fn start(&self) -> &chrono::DateTime<chrono::Utc> {
        self.times
            .last()
            .expect("invariant violation: times must be non-empty")
    }

    /// The last date of the streak
    pub fn end(&self) -> &chrono::DateTime<chrono::Utc> {
        self.times
            .first()
            .expect("invariant violation: times must be non-empty")
    }
}
