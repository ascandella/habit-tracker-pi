use db::{AccessLayer, DataAccessError};

use crate::TrackerDisplay;

pub struct HabitInterface<'tz, T: TrackerDisplay, TZ: chrono::TimeZone> {
    display: T,
    db: AccessLayer,
    timezone: &'tz TZ,
}

impl<'tz, T, TZ> HabitInterface<'tz, T, TZ>
where
    T: TrackerDisplay,
    TZ: chrono::TimeZone,
{
    pub fn new(display: T, db: AccessLayer, timezone: &'tz TZ) -> HabitInterface<'tz, T, TZ> {
        HabitInterface {
            display,
            db,
            timezone,
        }
    }

    pub fn refresh_stats(&mut self) -> Result<(), DataAccessError> {
        let current = self.db.current_streak(self.timezone)?;
        let previous = self.db.previous_streak(self.timezone, &current)?;

        self.display.display_streak(&current, &previous);

        Ok(())
    }

    pub fn shutdown(mut self) -> Result<(), DataAccessError> {
        self.display.clear_and_shutdown();
        self.db.close()
    }

    pub fn button_pressed(&mut self) -> Result<(), DataAccessError> {
        self.db.record_event()?;
        self.refresh_stats()
    }
}
