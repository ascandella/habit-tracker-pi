use crossbeam_channel::{select, unbounded};
use std::error::Error;
use std::time::Duration;

use ctrlc;
use rppal::gpio::{Gpio, Trigger};

mod button;
use button::DebouncedButton;

mod display;
use display::Display;

const GPIO_BUTTON: u8 = 26;
// Raspberry pi default GPIO cdev
const GPIO_CHIP: &str = "/dev/gpiochip0";

fn main() -> Result<(), Box<dyn Error>> {
    let (exit_tx, exit_rx) = unbounded();
    let (button_tx, button_rx) = unbounded();
    let gpio = Gpio::new()?;
    let mut pin = gpio.get(GPIO_BUTTON)?.into_input();

    let mut button = DebouncedButton::new(button_tx, Duration::from_millis(500));

    pin.set_async_interrupt(Trigger::FallingEdge, move |_| button.pressed())
        .expect("Could not set async interrupt on pin");

    ctrlc::set_handler(move || exit_tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");

    let mut display = Display::new(GPIO_CHIP);
    display.text("Hello, world", display.height() / 2, display.width() / 2);

    let mut running = true;
    let mut presses = 0;
    while running {
        select! {
            recv(button_rx) -> _ => {
                println!("Button pressed");
                presses += 1;
                display.wake_up();
                display.clear();
                display.text(format!("Presses: {}", presses).as_str(), display.height() / 2, display.width() / 2);
                display.sleep().expect("Unable to sleep");

            }
            recv(exit_rx) -> _ => {
                println!("Received control-c. Exiting...");
                running = false;
            }
        }
    }

    display.wake_up();
    display.clear_and_shutdown();

    Ok(())
}
