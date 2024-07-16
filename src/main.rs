use std::error::Error;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use ctrlc;
use rppal::gpio::{Gpio, Trigger};

mod button;
use button::DebouncedButton;

mod display;
use display::Display;

const GPIO_BUTTON: u8 = 26;

fn main() -> Result<(), Box<dyn Error>> {
    let (exit_tx, exit_rx) = channel();
    let (button_tx, button_rx) = channel();
    let gpio = Gpio::new()?;
    let mut pin = gpio.get(GPIO_BUTTON)?.into_input();

    let mut button = DebouncedButton::new(button_tx, Duration::from_millis(500));

    pin.set_async_interrupt(Trigger::FallingEdge, move |_| button.pressed())
        .expect("Could not set async interrupt on pin");

    ctrlc::set_handler(move || exit_tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");

    thread::spawn(move || loop {
        if let Ok(_) = button_rx.recv() {
            println!("Button pressed!");
        }
    });

    let mut display = Display::new();
    display.text("Hello, world", display.height() / 2, display.width() / 2);
    display.sleep().expect("Unable to sleep");

    exit_rx.recv().expect("Could not receive from channel.");
    println!("Received control-c. Exiting...");

    display.wake_up();
    display.clear();
    display.text("Good-bye", display.height() / 2, 10);

    // *Always* sleep before exiting the program
    display.sleep().expect("Unable to sleep");

    Ok(())
}
