use std::{
    sync::Mutex,
    time::{Duration, SystemTime},
};

pub(crate) struct DebouncedButton {
    tx: crossbeam_channel::Sender<()>,
    duration: Duration,
    last_press: Mutex<Option<SystemTime>>,
}

impl DebouncedButton {
    pub fn new(tx: crossbeam_channel::Sender<()>, debounce_duration: Duration) -> DebouncedButton {
        DebouncedButton {
            tx,
            duration: debounce_duration,
            last_press: Mutex::new(None),
        }
    }

    pub fn pressed(&mut self) {
        let mut last_press = self.last_press.lock().expect("Unable to acquire mutex");
        let should_fire = match *last_press {
            Some(last_press) => match last_press.elapsed() {
                Ok(elapsed) => elapsed > self.duration,
                Err(_) => {
                    eprintln!("Unable to check elapsed time for debounce");
                    true
                }
            },
            None => true,
        };
        if should_fire {
            self.tx
                .send(())
                .expect("Unable to send to button-fired channel");
            *last_press = Some(SystemTime::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_debounced_button() {
        let (tx, rx) = crossbeam_channel::bounded(5);
        let debounce_duration = Duration::from_millis(5);
        let mut button = DebouncedButton::new(tx, debounce_duration);
        // Fire the button press
        button.pressed();
        // Should not fire again
        button.pressed();
        assert_eq!(rx.try_recv().is_ok(), true);
        assert_eq!(rx.try_recv().is_err(), true);
        // Wait for debounce duration
        thread::sleep(debounce_duration);
        // Should fire again
        button.pressed();
        assert_eq!(rx.try_recv().is_ok(), true);
        assert_eq!(rx.try_recv().is_err(), true);
    }
}
