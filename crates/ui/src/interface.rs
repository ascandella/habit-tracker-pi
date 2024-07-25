use db::AccessLayer;

use crate::TrackerDisplay;

pub struct HabitInterface<T: TrackerDisplay> {
    display: T,
    db: AccessLayer,
}

impl<T: TrackerDisplay> HabitInterface<T> {
    pub fn new(display: T, db: AccessLayer) -> HabitInterface<T> {
        HabitInterface { display, db }
    }
}
