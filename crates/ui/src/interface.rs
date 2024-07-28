use db::{AccessLayer, DataAccessError};
use tracing::info;

use crate::TrackerDisplay;

pub struct HabitInterface<T: TrackerDisplay, TZ: chrono::TimeZone> {
    display: T,
    db: AccessLayer,
    timezone: TZ,
}

impl<T, TZ> HabitInterface<T, TZ>
where
    T: TrackerDisplay,
    TZ: chrono::TimeZone,
{
    pub fn new(display: T, db: AccessLayer, timezone: TZ) -> HabitInterface<T, TZ> {
        HabitInterface {
            display,
            db,
            timezone,
        }
    }

    pub fn refresh_stats(&mut self) -> Result<(), DataAccessError> {
        let current = self.db.current_streak(&self.timezone)?;
        let previous = self.db.previous_streak(&self.timezone, &current)?;

        self.display
            .display_streak(&self.timezone, &current, &previous);

        Ok(())
    }

    pub fn sleep(&mut self) {
        self.display.clear_and_shutdown();
    }

    pub fn shutdown(&mut self) -> Result<(), DataAccessError> {
        info!("Shutting down interface");
        self.display.clear_and_shutdown();
        Ok(())
    }

    pub fn button_pressed(&mut self) -> Result<(), DataAccessError> {
        info!("Button pressed");
        self.db.record_event()?;
        self.refresh_stats()
    }
}
