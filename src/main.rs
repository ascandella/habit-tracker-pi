use std::error::Error;
use std::sync::mpsc::channel;

use ctrlc;
use rppal::gpio::{Gpio, Trigger};

const GPIO_BUTTON: u8 = 26;

fn main() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = channel::<()>();
    let gpio = Gpio::new()?;
    let mut pin = gpio.get(GPIO_BUTTON)?.into_input();

    pin.set_async_interrupt(Trigger::FallingEdge, move |_level| {
        println!("Button pressed")
    })
    .expect("Could not set async interrupt on pin.");

    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");
    rx.recv().expect("Could not receive from channel.");
    println!("Got it! Exiting...");

    Ok(())
}
