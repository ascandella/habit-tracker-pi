use db::StreakData;

pub trait TrackerDisplay {
    /// For E-Paper displays, clear the screen and turn it off
    fn clear_and_shutdown(&mut self);

    /// Display the current and previous streak
    fn display_streak(&mut self, current: &StreakData, previous: &StreakData);
}

// TODO: Implement web-based TrackerDisplay
