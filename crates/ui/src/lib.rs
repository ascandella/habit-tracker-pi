use db::StreakData;

pub trait TrackerDisplay {
    /// For E-Paper displays, clear the screen and turn it off
    fn clear_and_shutdown(&mut self);

    /// Display the current and previous streak
    fn display_streak(
        &mut self,
        timezone: &impl chrono::TimeZone,
        current: &StreakData,
        previous: &StreakData,
    );
}

mod button;
mod interface;
pub use button::DebouncedButton;
pub use interface::HabitInterface;

// TODO: Implement web-based TrackerDisplay
