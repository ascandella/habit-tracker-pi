use crossbeam_channel::{bounded, select};
use gpiocdev::line::EdgeDetection;
use std::error::Error;
use std::time::Duration;

mod button;
use button::DebouncedButton;

mod display;
use display::Display;

const GPIO_BUTTON: u32 = 26;
// Raspberry pi default GPIO cdev
const GPIO_CHIP: &str = "/dev/gpiochip0";

fn main() -> Result<(), Box<dyn Error>> {
    let (button_tx, button_rx) = bounded(1);

    let pin_req = gpiocdev::Request::builder()
        .on_chip(GPIO_CHIP)
        .with_consumer("workout tracker")
        .with_line(GPIO_BUTTON)
        .with_edge_detection(EdgeDetection::FallingEdge)
        .request()?;

    let mut button = DebouncedButton::new(button_tx, Duration::from_millis(500));

    std::thread::spawn(move || {
        for _event in pin_req.edge_events() {
            button.pressed();
        }
    });

    let (exit_tx, exit_rx) = bounded(1);

    ctrlc::set_handler(move || exit_tx.send(()).expect("Could not send signal on channel"))?;

    let mut display = Display::new(GPIO_CHIP);
    display.text("Hello, world", display.height() / 2, display.width() / 2);
    display.sleep()?;

    let mut running = true;
    let mut presses = 0;
    while running {
        select! {
            recv(button_rx) -> _ => {
                presses += 1;
                display.wake_up();
                display.clear();
                display.text(format!("Presses: {}", presses).as_str(), display.height() / 2, display.width() / 2);
                display.sleep()?;
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